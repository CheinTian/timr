use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;
use strum::Display;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    symbols::shade,
    widgets::StatefulWidget,
};

use crate::{
    duration::{
        DurationEx, MINS_PER_HOUR, ONE_DECI_SECOND, ONE_HOUR, ONE_MINUTE, ONE_SECOND,
        SECS_PER_MINUTE,
    },
    utils::center_horizontal,
};

// max. 99:59:59
const MAX_DURATION: Duration =
    Duration::from_secs(100 * MINS_PER_HOUR * SECS_PER_MINUTE).saturating_sub(ONE_SECOND);

#[derive(Debug, Copy, Clone, Display, PartialEq, Eq)]
pub enum Time {
    Decis,
    Seconds,
    Minutes,
    Hours,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Initial,
    Tick,
    Pause,
    Editable(
        Time,
        Box<Mode>, /* previous mode before starting editing */
    ),
    Done,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Initial => write!(f, "[]"),
            Mode::Tick => write!(f, ">"),
            Mode::Pause => write!(f, "||"),
            Mode::Editable(time, _) => match time {
                Time::Decis => write!(f, "[edit deciseconds]"),
                Time::Seconds => write!(f, "[edit seconds]"),
                Time::Minutes => write!(f, "[edit minutes]"),
                Time::Hours => write!(f, "[edit hours]"),
            },
            Mode::Done => write!(f, "done"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Display, PartialOrd, Ord)]
pub enum Format {
    S,
    Ss,
    MSs,
    MmSs,
    HMmSs,
    HhMmSs,
}

#[derive(Debug, Copy, Clone, ValueEnum, Default, Serialize, Deserialize)]
pub enum Style {
    #[default]
    #[value(name = "full", alias = "f")]
    Full,
    #[value(name = "light", alias = "l")]
    Light,
    #[value(name = "medium", alias = "m")]
    Medium,
    #[value(name = "dark", alias = "d")]
    Dark,
    #[value(name = "thick", alias = "t")]
    Thick,
    #[value(name = "cross", alias = "c")]
    Cross,
    /// https://en.wikipedia.org/wiki/Braille_Patterns
    /// Note: Might not be supported in all terminals
    /// see https://docs.rs/ratatui/latest/src/ratatui/symbols.rs.html#150
    #[value(name = "braille", alias = "b")]
    Braille,
}

impl Style {
    pub fn next(&self) -> Self {
        match self {
            Style::Full => Style::Dark,
            Style::Dark => Style::Medium,
            Style::Medium => Style::Light,
            Style::Light => Style::Braille,
            Style::Braille => Style::Thick,
            Style::Thick => Style::Cross,
            Style::Cross => Style::Full,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Clock<T> {
    initial_value: DurationEx,
    current_value: DurationEx,
    tick_value: DurationEx,
    mode: Mode,
    format: Format,
    pub style: Style,
    pub with_decis: bool,
    phantom: PhantomData<T>,
}

pub struct ClockArgs {
    pub initial_value: Duration,
    pub current_value: Duration,
    pub tick_value: Duration,
    pub style: Style,
    pub with_decis: bool,
}

impl<T> Clock<T> {
    pub fn toggle_pause(&mut self) {
        self.mode = if self.mode == Mode::Tick {
            Mode::Pause
        } else {
            Mode::Tick
        }
    }

    pub fn get_initial_value(&self) -> &DurationEx {
        &self.initial_value
    }

    pub fn get_current_value(&self) -> &DurationEx {
        &self.current_value
    }

    pub fn toggle_edit(&mut self) {
        self.mode = match self.mode.clone() {
            Mode::Editable(_, prev) => {
                let p = *prev;
                // special cases: Should `Mode` be updated?
                // 1. `Done` -> `Initial` ?
                if p == Mode::Done && self.current_value.gt(&Duration::ZERO.into()) {
                    Mode::Initial
                }
                // 2. `_` -> `Done` ?
                else if p != Mode::Done && self.current_value.eq(&Duration::ZERO.into()) {
                    Mode::Done
                }
                // 3. `_` -> `_` (no change)
                else {
                    p
                }
            }
            mode => {
                if self.format <= Format::Ss {
                    Mode::Editable(Time::Seconds, Box::new(mode))
                } else {
                    Mode::Editable(Time::Minutes, Box::new(mode))
                }
            }
        };
    }

    pub fn edit_current_up(&mut self) {
        self.current_value = match self.mode {
            Mode::Editable(Time::Decis, _) => {
                if self
                    .current_value
                    // < 99:59:58
                    .le(&MAX_DURATION.saturating_sub(ONE_DECI_SECOND).into())
                {
                    self.current_value.saturating_add(ONE_DECI_SECOND.into())
                } else {
                    self.current_value
                }
            }
            Mode::Editable(Time::Seconds, _) => {
                if self
                    .current_value
                    // < 99:59:58
                    .le(&MAX_DURATION.saturating_sub(ONE_SECOND).into())
                {
                    self.current_value.saturating_add(ONE_SECOND.into())
                } else {
                    self.current_value
                }
            }
            Mode::Editable(Time::Minutes, _) => {
                if self
                    .current_value
                    // < 99:58:59
                    .le(&MAX_DURATION.saturating_sub(ONE_MINUTE).into())
                {
                    self.current_value.saturating_add(ONE_MINUTE.into())
                } else {
                    self.current_value
                }
            }
            Mode::Editable(Time::Hours, _) => {
                if self
                    .current_value
                    // < 98:59:59
                    .lt(&MAX_DURATION.saturating_sub(ONE_HOUR).into())
                {
                    self.current_value.saturating_add(ONE_HOUR.into())
                } else {
                    self.current_value
                }
            }
            _ => self.current_value,
        };
        self.update_format();
    }
    pub fn edit_current_down(&mut self) {
        self.current_value = match self.mode {
            Mode::Editable(Time::Decis, _) => {
                self.current_value.saturating_sub(ONE_DECI_SECOND.into())
            }
            Mode::Editable(Time::Seconds, _) => {
                self.current_value.saturating_sub(ONE_SECOND.into())
            }
            Mode::Editable(Time::Minutes, _) => {
                self.current_value.saturating_sub(ONE_MINUTE.into())
            }
            Mode::Editable(Time::Hours, _) => self.current_value.saturating_sub(ONE_HOUR.into()),
            _ => self.current_value,
        };
        self.update_format();
        self.update_mode();
    }

    pub fn get_mode(&self) -> &Mode {
        &self.mode
    }

    pub fn is_running(&self) -> bool {
        self.mode == Mode::Tick
    }

    pub fn is_edit_mode(&self) -> bool {
        matches!(self.mode, Mode::Editable(_, _))
    }

    fn edit_mode_next(&mut self) {
        let mode = self.mode.clone();
        self.mode = match mode {
            Mode::Editable(Time::Decis, prev) => Mode::Editable(Time::Seconds, prev),
            Mode::Editable(Time::Seconds, prev) if self.format <= Format::Ss && self.with_decis => {
                Mode::Editable(Time::Decis, prev)
            }
            Mode::Editable(Time::Seconds, prev) if self.format <= Format::Ss => {
                Mode::Editable(Time::Seconds, prev)
            }
            Mode::Editable(Time::Seconds, prev) => Mode::Editable(Time::Minutes, prev),
            Mode::Editable(Time::Minutes, prev)
                if self.format <= Format::MmSs && self.with_decis =>
            {
                Mode::Editable(Time::Decis, prev)
            }
            Mode::Editable(Time::Minutes, prev) if self.format <= Format::MmSs => {
                Mode::Editable(Time::Seconds, prev)
            }
            Mode::Editable(Time::Minutes, prev) => Mode::Editable(Time::Hours, prev),
            Mode::Editable(Time::Hours, prev) if self.with_decis => {
                Mode::Editable(Time::Decis, prev)
            }
            Mode::Editable(Time::Hours, prev) => Mode::Editable(Time::Seconds, prev),
            _ => mode,
        };
        self.update_format();
    }

    fn edit_mode_prev(&mut self) {
        let mode = self.mode.clone();
        self.mode = match mode {
            Mode::Editable(Time::Decis, prev) if self.format <= Format::Ss => {
                Mode::Editable(Time::Seconds, prev)
            }
            Mode::Editable(Time::Decis, prev) if self.format <= Format::MmSs => {
                Mode::Editable(Time::Minutes, prev)
            }
            Mode::Editable(Time::Decis, prev) if self.format <= Format::HhMmSs => {
                Mode::Editable(Time::Hours, prev)
            }
            Mode::Editable(Time::Seconds, prev) if self.with_decis => {
                Mode::Editable(Time::Decis, prev)
            }
            Mode::Editable(Time::Seconds, prev) if self.format <= Format::Ss => {
                Mode::Editable(Time::Seconds, prev)
            }
            Mode::Editable(Time::Seconds, prev) if self.format <= Format::MmSs => {
                Mode::Editable(Time::Minutes, prev)
            }
            Mode::Editable(Time::Seconds, prev) if self.format <= Format::HhMmSs => {
                Mode::Editable(Time::Hours, prev)
            }
            Mode::Editable(Time::Minutes, prev) => Mode::Editable(Time::Seconds, prev),
            Mode::Editable(Time::Hours, prev) => Mode::Editable(Time::Minutes, prev),
            _ => mode,
        };
        self.update_format();
    }

    fn update_mode(&mut self) {
        let mode = self.mode.clone();
        self.mode = match mode {
            Mode::Editable(Time::Hours, prev) if self.format <= Format::MmSs => {
                Mode::Editable(Time::Minutes, prev)
            }
            Mode::Editable(Time::Minutes, prev) if self.format <= Format::Ss => {
                Mode::Editable(Time::Seconds, prev)
            }
            _ => mode,
        }
    }

    pub fn reset(&mut self) {
        self.mode = Mode::Initial;
        self.current_value = self.initial_value;
        self.update_format();
    }

    pub fn is_done(&self) -> bool {
        self.mode == Mode::Done
    }

    fn update_format(&mut self) {
        self.format = self.get_format();
    }

    pub fn get_format(&self) -> Format {
        if self.current_value.hours() >= 10 {
            Format::HhMmSs
        } else if self.current_value.hours() >= 1 {
            Format::HMmSs
        } else if self.current_value.minutes() >= 10 {
            Format::MmSs
        } else if self.current_value.minutes() >= 1 {
            Format::MSs
        } else if self.current_value.seconds() >= 10 {
            Format::Ss
        } else {
            Format::S
        }
    }
}

#[derive(Debug, Clone)]
pub struct Countdown {}

impl Clock<Countdown> {
    pub fn new(args: ClockArgs) -> Self {
        let ClockArgs {
            initial_value,
            current_value,
            tick_value,
            style,
            with_decis,
        } = args;
        let mut instance = Self {
            initial_value: initial_value.into(),
            current_value: current_value.into(),
            tick_value: tick_value.into(),
            mode: if current_value == Duration::ZERO {
                Mode::Done
            } else if current_value == initial_value {
                Mode::Initial
            } else {
                Mode::Pause
            },
            format: Format::S,
            style,
            with_decis,
            phantom: PhantomData,
        };
        // update format once
        instance.update_format();
        instance
    }

    pub fn tick(&mut self) {
        if self.mode == Mode::Tick {
            self.current_value = self.current_value.saturating_sub(self.tick_value);
            self.set_done();
            self.update_format();
        }
    }

    fn set_done(&mut self) {
        if self.current_value.eq(&Duration::ZERO.into()) {
            self.mode = Mode::Done;
        }
    }

    pub fn get_percentage_done(&self) -> u16 {
        let elapsed = self.initial_value.saturating_sub(self.current_value);

        (elapsed.millis() * 100 / self.initial_value.millis()) as u16
    }

    pub fn edit_next(&mut self) {
        self.edit_mode_next();
    }

    pub fn edit_prev(&mut self) {
        self.edit_mode_prev();
    }

    pub fn edit_up(&mut self) {
        self.edit_current_up();
        // re-align `current_value` if needed
        if self.initial_value.lt(&self.current_value) {
            self.current_value = self.initial_value;
        }
    }

    pub fn edit_down(&mut self) {
        self.edit_current_down();
    }
}

#[derive(Debug, Clone)]
pub struct Timer {}

impl Clock<Timer> {
    pub fn new(args: ClockArgs) -> Self {
        let ClockArgs {
            initial_value,
            current_value,
            tick_value,
            style,
            with_decis,
        } = args;
        let mut instance = Self {
            initial_value: initial_value.into(),
            current_value: current_value.into(),
            tick_value: tick_value.into(),
            mode: if current_value == initial_value {
                Mode::Initial
            } else if current_value >= MAX_DURATION {
                Mode::Done
            } else {
                Mode::Pause
            },
            format: Format::S,
            phantom: PhantomData,
            style,
            with_decis,
        };
        // update format once
        instance.update_format();
        instance
    }

    pub fn tick(&mut self) {
        if self.mode == Mode::Tick {
            self.current_value = self.current_value.saturating_add(self.tick_value);
            self.set_done();
            self.update_format();
        }
    }

    fn set_done(&mut self) {
        if self.current_value.ge(&MAX_DURATION.into()) {
            self.mode = Mode::Done;
        }
    }

    pub fn edit_next(&mut self) {
        self.edit_mode_next();
    }

    pub fn edit_prev(&mut self) {
        self.edit_mode_prev();
    }

    pub fn edit_up(&mut self) {
        self.edit_current_up();
    }

    pub fn edit_down(&mut self) {
        self.edit_current_down();
    }
}

const DIGIT_SIZE: usize = 5;
const DIGIT_WIDTH: u16 = DIGIT_SIZE as u16;
const DIGIT_HEIGHT: u16 = DIGIT_SIZE as u16 + 1 /* border height */;
const COLON_WIDTH: u16 = 4; // incl. padding left + padding right
const SPACE_WIDTH: u16 = 1;

#[rustfmt::skip]
const DIGIT_0: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_1: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
];

#[rustfmt::skip]
const DIGIT_2: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    1, 1, 1, 1, 1,
    1, 1, 0, 0, 0,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_3: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_4: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 0, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
];

#[rustfmt::skip]
const DIGIT_5: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 0, 0,
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_6: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 0, 0,
    1, 1, 1, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_7: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
    0, 0, 0, 1, 1,
];

#[rustfmt::skip]
const DIGIT_8: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_9: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 1, 1,
    1, 1, 1, 1, 1,
    0, 0, 0, 1, 1,
    1, 1, 1, 1, 1,
];

#[rustfmt::skip]
const DIGIT_ERROR: [u8; DIGIT_SIZE * DIGIT_SIZE] = [
    1, 1, 1, 1, 1,
    1, 1, 0, 0, 0,
    1, 1, 1, 1, 0,
    1, 1, 0, 0, 0,
    1, 1, 1, 1, 1,
];

pub struct ClockWidget<T>
where
    T: std::fmt::Debug,
{
    phantom: PhantomData<T>,
}

impl<T> ClockWidget<T>
where
    T: std::fmt::Debug,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }

    fn get_digit_symbol(&self, style: &Style) -> &str {
        match &style {
            Style::Full => shade::FULL,
            Style::Light => shade::LIGHT,
            Style::Medium => shade::MEDIUM,
            Style::Dark => shade::DARK,
            Style::Cross => "╬",
            Style::Thick => "┃",
            Style::Braille => "⣿",
        }
    }

    fn get_horizontal_lengths(&self, format: &Format, with_decis: bool) -> Vec<u16> {
        let add_decis = |mut lengths: Vec<u16>, with_decis: bool| -> Vec<u16> {
            if with_decis {
                lengths.extend_from_slice(&[
                    COLON_WIDTH, // .
                    DIGIT_WIDTH, // ds
                ])
            }
            lengths
        };

        match format {
            Format::HhMmSs => add_decis(
                vec![
                    DIGIT_WIDTH, // h
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // h
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // m
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // m
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // s
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
            Format::HMmSs => add_decis(
                vec![
                    DIGIT_WIDTH, // h
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // m
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // m
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // s
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
            Format::MmSs => add_decis(
                vec![
                    DIGIT_WIDTH, // m
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // m
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // s
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
            Format::MSs => add_decis(
                vec![
                    DIGIT_WIDTH, // m
                    COLON_WIDTH, // :
                    DIGIT_WIDTH, // s
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
            Format::Ss => add_decis(
                vec![
                    DIGIT_WIDTH, // s
                    SPACE_WIDTH, // (space)
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
            Format::S => add_decis(
                vec![
                    DIGIT_WIDTH, // s
                ],
                with_decis,
            ),
        }
    }

    pub fn get_width(&self, format: &Format, with_decis: bool) -> u16 {
        self.get_horizontal_lengths(format, with_decis).iter().sum()
    }

    pub fn get_height(&self) -> u16 {
        DIGIT_HEIGHT
    }

    fn render_digit(
        &self,
        number: u64,
        symbol: &str,
        with_border: bool,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let left = area.left();
        let top = area.top();

        let symbols = match number {
            0 => DIGIT_0,
            1 => DIGIT_1,
            2 => DIGIT_2,
            3 => DIGIT_3,
            4 => DIGIT_4,
            5 => DIGIT_5,
            6 => DIGIT_6,
            7 => DIGIT_7,
            8 => DIGIT_8,
            9 => DIGIT_9,
            _ => DIGIT_ERROR,
        };

        symbols.iter().enumerate().for_each(|(i, item)| {
            let x = i % DIGIT_SIZE;
            let y = i / DIGIT_SIZE;
            if *item == 1 {
                let p = Position {
                    x: left + x as u16,
                    y: top + y as u16,
                };
                if let Some(cell) = buf.cell_mut(p) {
                    cell.set_symbol(symbol);
                }
            }
        });

        // Add border at the bottom
        if with_border {
            for x in 0..area.width {
                let p = Position {
                    x: left + x,
                    y: top + area.height - 1,
                };
                if let Some(cell) = buf.cell_mut(p) {
                    cell.set_symbol("─");
                }
            }
        }
    }

    fn render_colon(&self, symbol: &str, area: Rect, buf: &mut Buffer) {
        let left = area.left();
        let top = area.top();

        let positions = [
            Position {
                x: left + 1,
                y: top + 1,
            },
            Position {
                x: left + 2,
                y: top + 1,
            },
            Position {
                x: left + 1,
                y: top + 3,
            },
            Position {
                x: left + 2,
                y: top + 3,
            },
        ];

        for pos in positions {
            if let Some(cell) = buf.cell_mut(pos) {
                cell.set_symbol(symbol);
            }
        }
    }

    fn render_dot(&self, symbol: &str, area: Rect, buf: &mut Buffer) {
        let positions = [
            Position {
                x: area.left() + 1,
                y: area.top() + area.height - 2,
            },
            Position {
                x: area.left() + 2,
                y: area.top() + area.height - 2,
            },
        ];

        for pos in positions {
            if let Some(cell) = buf.cell_mut(pos) {
                cell.set_symbol(symbol);
            }
        }
    }
}

impl<T> StatefulWidget for ClockWidget<T>
where
    T: std::fmt::Debug,
{
    type State = Clock<T>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let with_decis = state.with_decis;
        let format = state.format;
        let symbol = self.get_digit_symbol(&state.style);
        let widths = self.get_horizontal_lengths(&format, with_decis);
        let area = center_horizontal(
            area,
            Constraint::Length(self.get_width(&format, with_decis)),
        );
        let edit_hours = matches!(state.mode, Mode::Editable(Time::Hours, _));
        let edit_minutes = matches!(state.mode, Mode::Editable(Time::Minutes, _));
        let edit_secs = matches!(state.mode, Mode::Editable(Time::Seconds, _));
        let edit_deci = matches!(state.mode, Mode::Editable(Time::Decis, _));
        match format {
            Format::HhMmSs if with_decis => {
                let [hh, _, h, c_hm, mm, _, m, c_ms, ss, _, s, d, ds] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.hours() / 10,
                    symbol,
                    edit_hours,
                    hh,
                    buf,
                );
                self.render_digit(state.current_value.hours() % 10, symbol, edit_hours, h, buf);
                self.render_colon(symbol, c_hm, buf);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::HhMmSs => {
                let [hh, _, h, c_hm, mm, _, m, c_ms, ss, _, s] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.hours() / 10,
                    symbol,
                    edit_hours,
                    hh,
                    buf,
                );
                self.render_digit(state.current_value.hours() % 10, symbol, edit_hours, h, buf);
                self.render_colon(symbol, c_hm, buf);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
            Format::HMmSs if with_decis => {
                let [h, c_hm, mm, _, m, c_ms, ss, _, s, d, ds] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(state.current_value.hours() % 10, symbol, edit_hours, h, buf);
                self.render_colon(symbol, c_hm, buf);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::HMmSs => {
                let [h, c_hm, mm, _, m, c_ms, ss, _, s] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(state.current_value.hours() % 10, symbol, edit_hours, h, buf);
                self.render_colon(symbol, c_hm, buf);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
            Format::MmSs if with_decis => {
                let [mm, _, m, c_ms, ss, _, s, d, ds] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::MmSs => {
                let [mm, _, m, c_ms, ss, _, s] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.minutes_mod() / 10,
                    symbol,
                    edit_minutes,
                    mm,
                    buf,
                );
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
            Format::MSs if with_decis => {
                let [m, c_ms, ss, _, s, d, ds] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::MSs => {
                let [m, c_ms, ss, _, s] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.minutes_mod() % 10,
                    symbol,
                    edit_minutes,
                    m,
                    buf,
                );
                self.render_colon(symbol, c_ms, buf);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
            Format::Ss if state.with_decis => {
                let [ss, _, s, d, ds] =
                    Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::Ss => {
                let [ss, _, s] = Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.seconds_mod() / 10,
                    symbol,
                    edit_secs,
                    ss,
                    buf,
                );
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
            Format::S if with_decis => {
                let [s, d, ds] = Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
                self.render_dot(symbol, d, buf);
                self.render_digit(state.current_value.decis(), symbol, edit_deci, ds, buf);
            }
            Format::S => {
                let [s] = Layout::horizontal(Constraint::from_lengths(widths)).areas(area);
                self.render_digit(
                    state.current_value.seconds_mod() % 10,
                    symbol,
                    edit_secs,
                    s,
                    buf,
                );
            }
        }
    }
}
