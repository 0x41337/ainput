use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use ainput::{Point, TouchDevice, TouchMultiplexer};

fn draw_ui(
    frame: &mut Frame,
    mux: &TouchMultiplexer,
    last_event: &str,
    last_key: &str,
    physical_events: u64,
) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Min(6),
        ])
        .split(frame.area());

    let touchscreen = mux.touchscreen();

    let display = mux.display_size();

    let x_range = touchscreen.x_range();

    let y_range = touchscreen.y_range();

    let tracking_range = touchscreen.tracking_id_range();

    /*
     * ========================================================
     * STATUS
     * ========================================================
     */

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "ainput",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" - touchscreen multiplexer"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Status"));

    frame.render_widget(title, areas[0]);

    /*
     * ========================================================
     * INPUT
     * ========================================================
     */

    let point = mux.virtual_position();

    let status = Paragraph::new(vec![
        Line::from(format!("Device: {}", touchscreen.name())),
        Line::from(format!("Path: {}", touchscreen.path().display())),
        Line::from(format!("Virtual: ({}, {})", point.x, point.y)),
        Line::from(format!(
            "Virtual touch: {}",
            if mux.virtual_touch_active() {
                "DOWN"
            } else {
                "UP"
            }
        )),
        Line::from(format!(
            "Physical contacts: {}",
            mux.physical_contact_count()
        )),
        Line::from(format!("Total contacts: {}", mux.contact_count())),
        Line::from(format!("Physical slots: {}", mux.physical_slot_count())),
        Line::from(format!("Virtual slot: {}", mux.virtual_slot())),
        Line::from(format!("Display: {}x{}", display.width, display.height)),
    ])
    .block(Block::default().borders(Borders::ALL).title("Input"));

    frame.render_widget(status, areas[1]);

    /*
     * ========================================================
     * CONTROLS
     * ========================================================
     */

    let controls = Paragraph::new(vec![
        Line::from("← →    move X"),
        Line::from("↑ ↓    move Y"),
        Line::from("Enter  virtual DOWN"),
        Line::from("Space  virtual UP"),
        Line::from("q      exit"),
    ])
    .block(Block::default().borders(Borders::ALL).title("Controls"));

    frame.render_widget(controls, areas[2]);

    /*
     * ========================================================
     * DEBUG
     * ========================================================
     */

    let debug = Paragraph::new(vec![
        Line::from(format!("X raw: {}..{}", x_range.min, x_range.max)),
        Line::from(format!("Y raw: {}..{}", y_range.min, y_range.max)),
        Line::from(format!(
            "Tracking: {}..{}",
            tracking_range.min, tracking_range.max
        )),
        Line::from(format!("UInput slots: {}", mux.total_slot_count())),
        Line::from(format!("Physical events: {}", physical_events)),
        Line::from(format!("Last key: {}", last_key)),
        Line::from(format!("Last event: {}", last_event)),
    ])
    .block(Block::default().borders(Borders::ALL).title("Debug"));

    frame.render_widget(debug, areas[3]);
}

fn run_app(terminal: &mut DefaultTerminal, mux: &mut TouchMultiplexer) -> io::Result<()> {
    let mut running = true;

    let mut last_event = String::from("-");

    let mut last_key = String::from("-");

    let mut physical_events = 0u64;

    while running {
        /*
         * Processes physical events.
         */
        physical_events += mux.poll()?;

        /*
         * Render.
         */
        terminal.draw(|frame| {
            draw_ui(frame, mux, &last_event, &last_key, physical_events);
        })?;

        /*
         * TUI keyboard.
         */
        if event::poll(Duration::from_millis(5))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                last_key = format!("{:?}", key.code);

                match key.code {
                    /*
                     * Exit.
                     */
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        running = false;

                        last_event = "exiting".to_string();
                    }

                    /*
                     * Move virtual left.
                     */
                    KeyCode::Left => {
                        let point = mux.virtual_position();

                        mux.touch_move(Point::new(point.x - 20, point.y))?;

                        let point = mux.virtual_position();

                        last_event = format!("X={}", point.x);
                    }

                    /*
                     * Move virtual right.
                     */
                    KeyCode::Right => {
                        let point = mux.virtual_position();

                        mux.touch_move(Point::new(point.x + 20, point.y))?;

                        let point = mux.virtual_position();

                        last_event = format!("X={}", point.x);
                    }

                    /*
                     * Move virtual up.
                     */
                    KeyCode::Up => {
                        let point = mux.virtual_position();

                        mux.touch_move(Point::new(point.x, point.y - 20))?;

                        let point = mux.virtual_position();

                        last_event = format!("Y={}", point.y);
                    }

                    /*
                     * Move virtual down.
                     */
                    KeyCode::Down => {
                        let point = mux.virtual_position();

                        mux.touch_move(Point::new(point.x, point.y + 20))?;

                        let point = mux.virtual_position();

                        last_event = format!("Y={}", point.y);
                    }

                    /*
                     * Enter = DOWN.
                     */
                    KeyCode::Enter => {
                        let point = mux.virtual_position();

                        mux.touch_down(point)?;

                        last_event = format!("VIRTUAL DOWN ({}, {})", point.x, point.y,);
                    }

                    /*
                     * Space = UP.
                     */
                    KeyCode::Char(' ') => {
                        mux.touch_up()?;

                        last_event = "VIRTUAL UP".to_string();
                    }

                    _ => {}
                }
            }
        }
    }

    /*
     * Safety:
     * never leave a virtual touch stuck.
     */
    mux.touch_up()?;

    Ok(())
}

fn main() -> io::Result<()> {
    /*
     * ========================================================
     * DETECTION
     * ========================================================
     */

    let touchscreen = TouchDevice::detect().map_err(io::Error::other)?;

    println!("[detect] {}", touchscreen.summary());

    /*
     * ========================================================
     * MULTIPLEXER
     * ========================================================
     */

    let mut mux = TouchMultiplexer::open(touchscreen)?;

    if let Some(path) = mux.output_evdev_path() {
        println!("[uinput] {}", path.display());
    }

    /*
     * ========================================================
     * TERMINAL
     * ========================================================
     */

    enable_raw_mode()?;

    execute!(io::stdout(), EnterAlternateScreen)?;

    let mut terminal = ratatui::init();

    let result = run_app(&mut terminal, &mut mux);

    /*
     * ========================================================
     * CLEANUP
     * ========================================================
     */

    ratatui::restore();

    let _ = disable_raw_mode();

    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    result
}
