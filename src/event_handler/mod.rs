mod tone_generator;

use std::cmp::PartialEq;
use crate::types::*;
use tone_generator::ToneGenerator;

use interception as ic;
use vigem::*;

use serde::{Deserialize, Serialize};

use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::hint::spin_loop;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bind {
    Keyboard(ic::ScanCode),
    Mouse(MouseButton),
    MouseMove,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ControllerAction {
    Button(ControllerButton),
    AnalogLeft(f64, f64),
    AnalogRight(f64, f64),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    sensitivity: f64,

    sample_window: Duration,

    spin_period: Duration,

    oversteer_alert_enabled: bool,
    oversteer_alert_threshold: f64,
    oversteer_alert: tone_generator::Config,

    analog_circularize: bool,
    mouse_button_fix: bool,

    binds: HashMap<Bind, ControllerAction>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sensitivity: 5.0,

            sample_window: Duration::from_millis(20),

            spin_period: Duration::from_millis(2),

            oversteer_alert_enabled: false,
            oversteer_alert_threshold: 1.5,
            oversteer_alert: tone_generator::Config::default(),

            analog_circularize: false,
            mouse_button_fix: false,

            binds: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum AnalogType {
    Left,
    Right,
}


#[derive(Debug)]
pub struct AnalogState {
    analog_type: AnalogType,
    x: f64,
    y: f64,
}

impl Display for AnalogType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalogType::Left => write!(f, "Left"),
            AnalogType::Right => write!(f, "Right")
        }
    }
}


impl Display for AnalogState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AnalogType: {}, x: {:.2}, y: {:.2}",
            self.analog_type, self.x, self.y
        )
    }
}

pub struct EventHandler {
    config: Config,

    rx: mpsc::Receiver<Event>,

    vigem: Vigem,
    target: Target,
    report: XUSBReport,

    tone_generator: Option<ToneGenerator>,

    mouse_samples: VecDeque<(i32, i32, Instant)>,
    mouse_button_states: (KeyState, KeyState),

    analog_state: HashMap<Bind, AnalogState>,
    iteration_count: i32,
    iteration_total: Duration,
    iteration_window_start: Instant,
}

impl EventHandler {
    const ANALOG_MAX: f64 = -(i16::MIN as f64);

    pub fn new(rx: mpsc::Receiver<Event>, _config: Config) -> Result<Self, anyhow::Error> {
        let mut config = _config;

        if !config.binds.contains_key(&Bind::MouseMove) {
            error!("MouseMove is not bound to any analog.\nTry to add:\n-> MouseMove: AnalogRight(1, -1) <-\n to the binds in your config.ron");
            config.binds.insert(Bind::MouseMove, ControllerAction::AnalogRight(1.0, -1.0));
        } else {
            let bind = config.binds.get_mut(&Bind::MouseMove).unwrap();
            match bind {
                ControllerAction::Button(_) => {
                    error!("MouseMove is not bound to any analog. Instead, it is bound to Button which is not allowed. Appropriate values are:\nAnalogRight(x, y) and AnalogLeft(x, y)");
                    *bind = ControllerAction::AnalogRight(1.0, -1.0);
                }
                _ => {}
            }
        }
        let mut vigem = Vigem::new();
        vigem.connect()?;

        let mut target = Target::new(TargetType::Xbox360);
        vigem.target_add(&mut target)?;

        info!("ViGEm connected, controller index: {}", target.index());

        info!(
            "sensitivity: {}, sample_window: {:#?}",
            config.sensitivity, config.sample_window,
        );

        let tone_generator = match config.oversteer_alert_enabled {
            true => Some(ToneGenerator::new(config.oversteer_alert)?),
            false => None,
        };

        Ok(EventHandler {
            config,

            rx,

            vigem,
            target,
            report: XUSBReport::default(),

            tone_generator,

            mouse_samples: VecDeque::new(),
            mouse_button_states: (KeyState::Up, KeyState::Up),

            analog_state: HashMap::new(),
            iteration_count: 0,
            iteration_total: Duration::from_secs(0),
            iteration_window_start: Instant::now(),
        })
    }

    pub fn run(&mut self) -> Result<(), anyhow::Error> {
        loop {
            let iteration_start = Instant::now();

            let mut event = self.rx.try_recv();
            while event.is_err() && iteration_start.elapsed() < self.config.spin_period {
                spin_loop();
                event = self.rx.try_recv();
            }
            // match event {
            //     Ok(x) => info!("{:?}", x),
            //     Err(_) => {}
            // }
            if let Ok(event) = event {
                match event {
                    Event::MouseMove(x, y) =>
                        {
                            self.handle_mouse_move(x, y)
                        }

                    Event::MouseButton(button, state) => {
                        if button == MouseButton::Left {
                            self.mouse_button_states.0 = state;
                        }

                        if button == MouseButton::Right {
                            self.mouse_button_states.1 = state;
                        }

                        self.handle_bind(Bind::Mouse(button), state);

                        if self.config.mouse_button_fix && state == KeyState::Up {
                            if self.mouse_button_states.0 == KeyState::Down {
                                self.handle_bind(Bind::Mouse(MouseButton::Left), KeyState::Down)
                            }

                            if self.mouse_button_states.1 == KeyState::Down {
                                self.handle_bind(Bind::Mouse(MouseButton::Right), KeyState::Down)
                            }
                        }
                    }

                    Event::Keyboard(scancode, state) => {
                        self.handle_bind(Bind::Keyboard(scancode), state)
                    }

                    Event::Reset => {
                        self.mouse_button_states = (KeyState::Up, KeyState::Up);
                        self.report = XUSBReport::default();
                    }
                }
            }

            self.update_analog();
            self.vigem.update(&self.target, &self.report)?;

            if log_enabled!(log::Level::Info) {
                self.iteration_count += 1;
                self.iteration_total += iteration_start.elapsed();

                if self.iteration_window_start.elapsed() > Duration::from_secs(2) {
                    debug!(
                        "{} loops, {} per sec, avg = {:#?}",
                        self.iteration_count,
                        self.iteration_count as f64 / 2.0,
                        self.iteration_total.div_f64(self.iteration_count.into())
                    );

                    self.iteration_count = 0;
                    self.iteration_total = Duration::from_secs(0);
                    self.iteration_window_start = Instant::now();
                }
            }
        }
    }

    fn handle_bind(&mut self, bind: Bind, state: KeyState) {
        let controller_button = match self.config.binds.get(&bind) {
            Some(ControllerAction::Button(controller_button)) => controller_button,
            Some(ControllerAction::AnalogLeft(x, y)) => {
                if self.analog_state.contains_key(&bind) && state == KeyState::Up
                {
                    self.analog_state.remove(&bind);
                    return;
                }

                self.analog_state.insert(bind, AnalogState {
                    analog_type: AnalogType::Left,
                    x: *x,
                    y: *y,
                });

                return;
            }
            Some(ControllerAction::AnalogRight(x, y)) => {
                if self.analog_state.contains_key(&bind) && state == KeyState::Up
                {
                    self.analog_state.remove(&bind);
                    return;
                }

                self.analog_state.insert(bind, AnalogState {
                    analog_type: AnalogType::Right,
                    x: *x,
                    y: *y,
                });
                return;
            }
            None => return,
        };

        match *controller_button {
            ControllerButton::LeftTrigger => match state {
                KeyState::Down => self.report.b_left_trigger = u8::MAX,
                KeyState::Up => self.report.b_left_trigger = 0,
            },

            ControllerButton::RightTrigger => match state {
                KeyState::Down => self.report.b_right_trigger = u8::MAX,
                KeyState::Up => self.report.b_right_trigger = 0,
            },

            button => {
                let button_flag = XButton::from_bits(button as u16).unwrap();

                match state {
                    KeyState::Down => self.report.w_buttons |= button_flag,
                    KeyState::Up => self.report.w_buttons &= !button_flag,
                }
            }
        }

        if state == KeyState::Up {
            return;
        }
    }
    fn handle_mouse_move(&mut self, x: i32, y: i32) {
        let now = Instant::now();
        self.mouse_samples.push_back((x, y, now));
    }
    fn get_mouse_move_bind(&mut self) -> AnalogState
    {
        let mut analog_state: AnalogState = AnalogState {
            analog_type: AnalogType::Left,
            x: 0.0,
            y: 0.0,
        };
        let bind = self.config.binds.get(&Bind::MouseMove).copied().unwrap();

        match bind {
            ControllerAction::AnalogLeft(x, y) => {
                analog_state.analog_type = AnalogType::Left;
                analog_state.x = x;
                analog_state.y = y;
            }
            ControllerAction::AnalogRight(x, y) => {
                analog_state.analog_type = AnalogType::Right;
                analog_state.x = x;
                analog_state.y = y;
            }
            _ => {}
        }

        analog_state
    }
    fn update_mouse_state(&mut self, mouse_vel: (f64, f64))
    {
        let mouse_bind = self.get_mouse_move_bind();
        if self.analog_state.contains_key(&Bind::MouseMove)
        {
            let state = self.analog_state.get_mut(&Bind::MouseMove).unwrap();
            state.x = mouse_bind.x * mouse_vel.0;
            state.y = mouse_bind.y * mouse_vel.1;
        } else {
            let state = AnalogState {
                analog_type: mouse_bind.analog_type,
                x: mouse_bind.x * mouse_vel.0,
                y: mouse_bind.y * mouse_vel.1,
            };
            self.analog_state.insert(Bind::MouseMove, state);
        }
    }
    fn update_analog(&mut self) {
        let now = Instant::now();

        loop {
            let sample = match self.mouse_samples.front() {
                Some(sample) => sample,
                None => break,
            };

            if now - sample.2 > self.config.sample_window {
                self.mouse_samples.pop_front();
            } else {
                break;
            }
        }


        let mut mouse_vel = (0.0, 0.0);

        for &(x, y, _) in self.mouse_samples.iter() {
            mouse_vel.0 += x as f64;
            mouse_vel.1 += y as f64;
        }

        let multiplier = self.config.sensitivity / (1e4 * self.config.sample_window.as_secs_f64());
        mouse_vel.0 *= multiplier;
        mouse_vel.1 *= multiplier;

        let mut states = (
            AnalogState {
                analog_type: AnalogType::Left,
                x: 0.0,
                y: 0.0,
            }, AnalogState {
                analog_type: AnalogType::Right,
                x: 0.0,
                y: 0.0,
            }
        );
        self.update_mouse_state(mouse_vel);

        for (_bind, state) in &self.analog_state {
            match state.analog_type {
                AnalogType::Left => {
                    states.0.x += state.x;
                    states.0.y += state.y;
                }
                AnalogType::Right => {
                    states.1.x += state.x;
                    states.1.y += state.y;
                }
            }
        }
        self.set_analog(states.0);
        self.set_analog(states.1);
    }

    fn set_analog(&mut self, state: AnalogState) {
        let alert = state.x.abs().max(state.y.abs()) >= self.config.oversteer_alert_threshold;
        self.tone_generator.as_mut().map(|tg| tg.enable(alert));

        if self.config.analog_circularize {
            self.set_analog_circularized(state);
        } else {
            self.set_analog_linear(state);
        }
    }

    fn set_analog_circularized(&mut self, state: AnalogState) {
        let angle = state.y.atan2(state.x);
        let radius = (state.x.powi(2) + state.y.powi(2)).sqrt();
        match state.analog_type {
            AnalogType::Left => {
                self.report.s_thumb_lx = (angle.cos() * radius * Self::ANALOG_MAX) as i16;
                self.report.s_thumb_ly = (angle.sin() * radius * Self::ANALOG_MAX) as i16;
            }
            AnalogType::Right => {
                self.report.s_thumb_rx = (angle.cos() * radius * Self::ANALOG_MAX) as i16;
                self.report.s_thumb_ry = (angle.sin() * radius * Self::ANALOG_MAX) as i16;
            }
        }
    }

    fn set_analog_linear(&mut self, state: AnalogState) {
        if state.x.abs() <= 1.0 && state.y.abs() <= 1.0 {
            match state.analog_type {
                AnalogType::Left => {
                    self.report.s_thumb_lx = (state.x * Self::ANALOG_MAX) as i16;
                    self.report.s_thumb_ly = (state.y * Self::ANALOG_MAX) as i16;
                }
                AnalogType::Right => {
                    self.report.s_thumb_rx = (state.x * Self::ANALOG_MAX) as i16;
                    self.report.s_thumb_ry = (state.y * Self::ANALOG_MAX) as i16;
                }
            }
            return;
        }

        let overshoot = state.x.abs().max(state.y.abs());

        let angle = state.y.atan2(state.x);
        let radius = (state.x.powi(2) + state.y.powi(2)).sqrt();

        let new_radius = radius / overshoot;

        match state.analog_type {
            AnalogType::Left => {
                self.report.s_thumb_lx = (angle.cos() * new_radius * Self::ANALOG_MAX) as i16;
                self.report.s_thumb_ly = (angle.sin() * new_radius * Self::ANALOG_MAX) as i16;
            }
            AnalogType::Right => {
                self.report.s_thumb_rx = (angle.cos() * new_radius * Self::ANALOG_MAX) as i16;
                self.report.s_thumb_ry = (angle.sin() * new_radius * Self::ANALOG_MAX) as i16;
            }
        }
    }
}
