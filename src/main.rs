use chrono::{Local, NaiveDate, NaiveTime};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, BorderType},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    content: String,
    completed: bool,
    date: Option<NaiveDate>,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ViewMode {
    Scheduled,
    Notes,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppData {
    tasks: Vec<Task>,
    #[serde(default)]
    notes: String,
}

impl AppData {
    fn new() -> Self {
        Self {
            tasks: Vec::new(),
            notes: String::new(),
        }
    }

    fn load() -> io::Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = format!("{}/.keep_tasks.json", home);

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                Ok(serde_json::from_str(&content).unwrap_or_else(|_| Self::new()))
            }
            Err(_) => Ok(Self::new()),
        }
    }

    fn save(&self) -> io::Result<()> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = format!("{}/.keep_tasks.json", home);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn tasks_for_date(&self, date: &NaiveDate) -> Vec<(usize, &Task)> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.date.as_ref() == Some(date))
            .collect()
    }

    fn overdue_tasks(&self, current_date: &NaiveDate) -> Vec<(usize, &Task)> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if let Some(task_date) = t.date {
                    task_date < *current_date && !t.completed
                } else {
                    false
                }
            })
            .collect()
    }
}

struct App {
    data: AppData,
    current_date: NaiveDate,
    selected_task: usize,
    input_mode: bool,
    input_buffer: String,
    start_time_buffer: String,
    end_time_buffer: String,
    time_input_field: usize, // 0 = task, 1 = start_time, 2 = end_time
    editing_task_idx: Option<usize>, // None = adding new task, Some(idx) = editing task
    notes_buffer: String,
    notes_cursor: usize, // Cursor position in notes buffer
    should_quit: bool,
    view_mode: ViewMode,
}

impl App {
    fn new() -> io::Result<Self> {
        let data = AppData::load()?;
        let notes_buffer = data.notes.clone();
        let notes_cursor = notes_buffer.len();
        Ok(Self {
            data,
            current_date: Local::now().date_naive(),
            selected_task: 0,
            input_mode: false,
            input_buffer: String::new(),
            start_time_buffer: String::new(),
            end_time_buffer: String::new(),
            time_input_field: 0,
            editing_task_idx: None,
            notes_buffer,
            notes_cursor,
            should_quit: false,
            view_mode: ViewMode::Scheduled,
        })
    }

    fn next_day(&mut self) {
        self.current_date = self.current_date.succ_opt().unwrap_or(self.current_date);
        self.selected_task = 0;
    }

    fn prev_day(&mut self) {
        self.current_date = self.current_date.pred_opt().unwrap_or(self.current_date);
        self.selected_task = 0;
    }

    fn current_tasks(&self) -> Vec<(usize, &Task)> {
        let mut tasks = match self.view_mode {
            ViewMode::Scheduled => self.data.tasks_for_date(&self.current_date),
            ViewMode::Notes => Vec::new(), // No tasks in notes view
        };

        // Sort by start time: tasks with start_time first (sorted), then tasks without
        tasks.sort_by(|a, b| {
            match (a.1.start_time, b.1.start_time) {
                (Some(time_a), Some(time_b)) => time_a.cmp(&time_b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        tasks
    }

    fn next_task(&mut self) {
        let tasks = self.current_tasks();
        if !tasks.is_empty() {
            self.selected_task = (self.selected_task + 1) % tasks.len();
        }
    }

    fn prev_task(&mut self) {
        let tasks = self.current_tasks();
        if !tasks.is_empty() {
            self.selected_task = if self.selected_task == 0 {
                tasks.len() - 1
            } else {
                self.selected_task - 1
            };
        }
    }

    fn toggle_task(&mut self) {
        let tasks = self.current_tasks();
        if let Some(&(idx, _)) = tasks.get(self.selected_task) {
            self.data.tasks[idx].completed = !self.data.tasks[idx].completed;
            let _ = self.data.save();
        }
    }

    fn start_edit_task(&mut self) {
        let tasks = self.current_tasks();
        if let Some(&(idx, _)) = tasks.get(self.selected_task) {
            // Clone the task data before dropping the borrow
            let task = self.data.tasks[idx].clone();

            self.input_buffer = task.content;
            self.start_time_buffer = task
                .start_time
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_default();
            self.end_time_buffer = task
                .end_time
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_default();
            self.editing_task_idx = Some(idx);
            self.input_mode = true;
            self.time_input_field = 0;
        }
    }

    fn add_task(&mut self) {
        if !self.input_buffer.trim().is_empty() {
            let start_time = if !self.start_time_buffer.trim().is_empty() {
                NaiveTime::parse_from_str(self.start_time_buffer.trim(), "%H:%M").ok()
            } else {
                None
            };

            let end_time = if !self.end_time_buffer.trim().is_empty() {
                NaiveTime::parse_from_str(self.end_time_buffer.trim(), "%H:%M").ok()
            } else {
                None
            };

            if let Some(idx) = self.editing_task_idx {
                // Editing existing task
                self.data.tasks[idx].content = self.input_buffer.trim().to_string();
                self.data.tasks[idx].start_time = start_time;
                self.data.tasks[idx].end_time = end_time;
            } else {
                // Adding new task - only in Scheduled view
                let date = Some(self.current_date);

                self.data.tasks.push(Task {
                    content: self.input_buffer.trim().to_string(),
                    completed: false,
                    date,
                    start_time,
                    end_time,
                });
            }
            let _ = self.data.save();
            self.input_buffer.clear();
            self.start_time_buffer.clear();
            self.end_time_buffer.clear();
        }
        self.input_mode = false;
        self.time_input_field = 0;
        self.editing_task_idx = None;
    }

    fn delete_task(&mut self) {
        let tasks = self.current_tasks();
        if let Some(&(idx, _)) = tasks.get(self.selected_task) {
            self.data.tasks.remove(idx);
            let _ = self.data.save();
            if self.selected_task > 0 {
                self.selected_task -= 1;
            }
        }
    }

    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Scheduled => ViewMode::Notes,
            ViewMode::Notes => ViewMode::Scheduled,
        };
        self.selected_task = 0;
    }

    fn save_notes(&mut self) {
        self.data.notes = self.notes_buffer.clone();
        let _ = self.data.save();
    }
}

fn main() -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new()?;

    let result = run_app(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;

    if let Err(err) = result {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(f.area());

            // Split the middle section into main area and sidebar
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(40),
                    Constraint::Length(35),
                ])
                .split(main_chunks[1]);

            // Calculate statistics
            let tasks = app.current_tasks();
            let total = tasks.len();
            let completed = tasks.iter().filter(|(_, t)| t.completed).count();
            let pending = total - completed;

            let (header_text, title, title_style) = match app.view_mode {
                ViewMode::Scheduled => {
                    let today = Local::now().date_naive();
                    let date_str = if app.current_date == today {
                        format!("📅 {} (Today)", app.current_date.format("%A, %B %d, %Y"))
                    } else {
                        format!("📅 {}", app.current_date.format("%A, %B %d, %Y"))
                    };
                    (date_str, "Scheduled Tasks", Style::default().fg(Color::Cyan).bold())
                }
                ViewMode::Notes => (
                    "📝 Free-form Notes & Ideas".to_string(),
                    "Notes",
                    Style::default().fg(Color::Rgb(150, 100, 200)).bold()
                ),
            };

            let stats = format!(" {} Total  •  {} Pending  •  {} Done ", total, pending, completed);

            let header_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(
                    Line::from(vec![
                        Span::styled("  Keep ", Style::default().fg(Color::White).bold()),
                        Span::styled("▸", Style::default().fg(Color::Cyan)),
                        Span::styled(" Task Manager  ", Style::default().fg(Color::DarkGray)),
                    ])
                )
                .title_alignment(Alignment::Left);

            let header_content = vec![
                Line::from(vec![
                    Span::styled(&header_text, title_style),
                    Span::raw("  "),
                    Span::styled(&stats, Style::default().fg(Color::DarkGray)),
                ]),
            ];

            let header = Paragraph::new(header_content)
                .block(header_block)
                .alignment(Alignment::Center);
            f.render_widget(header, main_chunks[0]);

            // Main content area - either tasks or notes
            if app.view_mode == ViewMode::Notes {
                // Notes view with visible cursor
                let text_with_cursor = if app.notes_buffer.is_empty() {
                    "█".to_string()
                } else {
                    let cursor_pos = app.notes_cursor.min(app.notes_buffer.len());
                    let (before, after) = app.notes_buffer.split_at(cursor_pos);
                    format!("{}█{}", before, after)
                };

                let notes_lines: Vec<Line> = text_with_cursor
                    .lines()
                    .map(|line| {
                        let spans: Vec<Span> = line.chars().map(|ch| {
                            if ch == '█' {
                                Span::styled(
                                    "█",
                                    Style::default().fg(Color::White)
                                )
                            } else {
                                Span::raw(ch.to_string())
                            }
                        }).collect();
                        Line::from(spans)
                    })
                    .collect();

                let notes_display = if app.notes_buffer.is_empty() {
                    vec![
                        Line::from(vec![
                            Span::styled("█", Style::default().fg(Color::White)),
                        ]),
                    ]
                } else {
                    notes_lines
                };

                let notes_widget = Paragraph::new(notes_display)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Rgb(150, 100, 200)))
                            .title(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(title, title_style),
                                Span::raw("  "),
                            ]))
                            .title_alignment(Alignment::Left)
                    )
                    .alignment(Alignment::Left);
                f.render_widget(notes_widget, content_chunks[0]);
            } else {
                // Tasks view
                let tasks = app.current_tasks();

                let rows: Vec<Row> = tasks
                .iter()
                .enumerate()
                .map(|(i, (_, task))| {
                    let (checkbox, checkbox_style) = if task.completed {
                        ("●", Style::default().fg(Color::Green))
                    } else {
                        ("○", Style::default().fg(Color::DarkGray))
                    };

                    let start_time_str = task
                        .start_time
                        .map(|t| format!("🕐 {}", t.format("%H:%M")))
                        .unwrap_or_else(|| "   --:--".to_string());
                    let end_time_str = task
                        .end_time
                        .map(|t| format!("🕐 {}", t.format("%H:%M")))
                        .unwrap_or_else(|| "   --:--".to_string());

                    let (row_style, content_style) = if i == app.selected_task {
                        (
                            Style::default().bg(Color::Rgb(40, 40, 60)),
                            Style::default().fg(Color::White).bold()
                        )
                    } else if task.completed {
                        (
                            Style::default(),
                            Style::default().fg(Color::DarkGray)
                        )
                    } else {
                        (
                            Style::default(),
                            Style::default().fg(Color::White)
                        )
                    };

                    Row::new(vec![
                        Cell::from(checkbox).style(checkbox_style),
                        Cell::from(start_time_str).style(if task.start_time.is_some() { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }),
                        Cell::from(end_time_str).style(if task.end_time.is_some() { Style::default().fg(Color::Magenta) } else { Style::default().fg(Color::DarkGray) }),
                        Cell::from(task.content.clone()).style(content_style),
                    ])
                    .style(row_style)
                    .height(1)
                })
                .collect();

            let header = Row::new(vec![
                Cell::from("  ").style(Style::default().fg(Color::Cyan).bold()),
                Cell::from("Start Time").style(Style::default().fg(Color::Cyan).bold()),
                Cell::from("End Time").style(Style::default().fg(Color::Magenta).bold()),
                Cell::from("Task Description").style(Style::default().fg(Color::White).bold()),
            ])
            .height(1)
            .bottom_margin(1);

            let title_line = Line::from(vec![
                Span::raw("  "),
                Span::styled(title, title_style),
                Span::raw("  "),
            ]);

            let tasks_table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Min(30),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(100, 100, 120)))
                    .title(title_line)
                    .title_alignment(Alignment::Left)
            )
            .column_spacing(2);
                f.render_widget(tasks_table, content_chunks[0]);
            }

            // Overdue sidebar
            let overdue_tasks = app.data.overdue_tasks(&Local::now().date_naive());
            let overdue_count = overdue_tasks.len();

            let overdue_items: Vec<Line> = if overdue_tasks.is_empty() {
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  🎉 All caught up!",
                        Style::default().fg(Color::Green)
                    )),
                ]
            } else {
                overdue_tasks
                    .iter()
                    .take(10)
                    .map(|(_, task)| {
                        let date_str = task
                            .date
                            .map(|d| d.format("%b %d").to_string())
                            .unwrap_or_else(|| "---".to_string());

                        let task_preview = if task.content.len() > 25 {
                            format!("{}...", &task.content[..22])
                        } else {
                            task.content.clone()
                        };

                        Line::from(vec![
                            Span::styled("⚠ ", Style::default().fg(Color::Red)),
                            Span::styled(date_str, Style::default().fg(Color::Red)),
                            Span::raw(" "),
                            Span::styled(task_preview, Style::default().fg(Color::White)),
                        ])
                    })
                    .collect()
            };

            let sidebar_title = if overdue_count > 0 {
                format!("  ⚠️  Overdue ({})  ", overdue_count)
            } else {
                "  ✓ Overdue  ".to_string()
            };

            let sidebar_border_color = if overdue_count > 0 {
                Color::Red
            } else {
                Color::Green
            };

            let overdue_sidebar = Paragraph::new(overdue_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(sidebar_border_color))
                        .title(Line::from(vec![
                            Span::styled(sidebar_title, Style::default().fg(sidebar_border_color).bold()),
                        ]))
                        .title_alignment(Alignment::Left)
                )
                .alignment(Alignment::Left);
            f.render_widget(overdue_sidebar, content_chunks[1]);

            let help_block = if app.view_mode == ViewMode::Notes && !app.input_mode {
                let controls_line = Line::from(vec![
                    Span::styled(" ↑↓←→ ", Style::default().bg(Color::Rgb(80, 80, 100)).fg(Color::White)),
                    Span::raw(" Navigate  "),
                    Span::styled(" Home/End ", Style::default().bg(Color::Rgb(80, 80, 100)).fg(Color::White)),
                    Span::raw(" Line  "),
                    Span::styled(" Ctrl+S ", Style::default().bg(Color::Cyan).fg(Color::Black)),
                    Span::raw(" Save  "),
                    Span::styled(" Tab ", Style::default().bg(Color::Magenta).fg(Color::White)),
                    Span::raw(" Tasks  "),
                    Span::styled(" q ", Style::default().bg(Color::Red).fg(Color::White)),
                    Span::raw(" Quit"),
                ]);

                Paragraph::new(vec![controls_line])
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Rgb(150, 100, 200)))
                            .title(Line::from(vec![
                                Span::raw("  "),
                                Span::styled("📝 Notes Editor", Style::default().fg(Color::Rgb(150, 100, 200)).bold()),
                                Span::raw("  "),
                            ]))
                            .title_alignment(Alignment::Left)
                    )
                    .alignment(Alignment::Left)
            } else if app.input_mode {
                let task_style = if app.time_input_field == 0 {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let start_time_style = if app.time_input_field == 1 {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let end_time_style = if app.time_input_field == 2 {
                    Style::default().fg(Color::Magenta).bold()
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let mode_text = if app.editing_task_idx.is_some() { "✏️  EDIT MODE" } else { "➕ ADD MODE" };
                let mode_color = if app.editing_task_idx.is_some() { Color::Yellow } else { Color::Green };

                let input_line = Line::from(vec![
                    Span::styled("Task: ", task_style),
                    Span::styled(&app.input_buffer, task_style),
                    Span::raw("  "),
                    Span::styled("│", Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled("Start: ", start_time_style),
                    Span::styled(&app.start_time_buffer, start_time_style),
                    Span::raw("  "),
                    Span::styled("│", Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled("End: ", end_time_style),
                    Span::styled(&app.end_time_buffer, end_time_style),
                ]);

                let controls_line = Line::from(vec![
                    Span::styled(" Tab ", Style::default().bg(Color::Rgb(60, 60, 80)).fg(Color::White)),
                    Span::raw(" Switch  "),
                    Span::styled(" Enter ", Style::default().bg(Color::Green).fg(Color::Black).bold()),
                    Span::raw(" Save  "),
                    Span::styled(" Esc ", Style::default().bg(Color::Red).fg(Color::White)),
                    Span::raw(" Cancel"),
                ]);

                Paragraph::new(vec![input_line, controls_line])
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(mode_color))
                            .title(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(mode_text, Style::default().fg(mode_color).bold()),
                                Span::raw("  "),
                            ]))
                            .title_alignment(Alignment::Left)
                    )
                    .alignment(Alignment::Left)
            } else {
                let mut controls = vec![
                    Span::styled(" n ", Style::default().bg(Color::Green).fg(Color::Black).bold()),
                    Span::raw(" New  "),
                    Span::styled(" e ", Style::default().bg(Color::Blue).fg(Color::White)),
                    Span::raw(" Edit  "),
                    Span::styled(" Space ", Style::default().bg(Color::Yellow).fg(Color::Black).bold()),
                    Span::raw(" Toggle  "),
                    Span::styled(" d ", Style::default().bg(Color::Red).fg(Color::White)),
                    Span::raw(" Delete  "),
                ];

                if app.view_mode == ViewMode::Scheduled {
                    controls.extend(vec![
                        Span::styled(" ← → ", Style::default().bg(Color::Cyan).fg(Color::Black)),
                        Span::raw(" Days  "),
                    ]);
                }

                controls.extend(vec![
                    Span::styled(" Tab ", Style::default().bg(Color::Magenta).fg(Color::White)),
                    Span::raw(" View  "),
                    Span::styled(" ↑ ↓ ", Style::default().bg(Color::Rgb(80, 80, 100)).fg(Color::White)),
                    Span::raw(" Navigate  "),
                    Span::styled(" q ", Style::default().bg(Color::Red).fg(Color::White)),
                    Span::raw(" Quit"),
                ]);

                Paragraph::new(Line::from(controls))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Rgb(100, 100, 120)))
                            .title(Line::from(vec![
                                Span::raw("  "),
                                Span::styled("⌨️  Controls", Style::default().fg(Color::White).bold()),
                                Span::raw("  "),
                            ]))
                            .title_alignment(Alignment::Left)
                    )
                    .alignment(Alignment::Left)
            };

            f.render_widget(help_block, main_chunks[2]);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                handle_input(app, key)?;
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_input(app: &mut App, key: KeyEvent) -> io::Result<()> {
    if app.view_mode == ViewMode::Notes && !app.input_mode {
        match key.code {
            KeyCode::Char('s') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                app.save_notes();
            }
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Tab => app.toggle_view(),
            KeyCode::Enter => {
                app.notes_buffer.insert(app.notes_cursor, '\n');
                app.notes_cursor += 1;
            }
            KeyCode::Char(c) => {
                app.notes_buffer.insert(app.notes_cursor, c);
                app.notes_cursor += 1;
            }
            KeyCode::Backspace => {
                if app.notes_cursor > 0 {
                    app.notes_cursor -= 1;
                    app.notes_buffer.remove(app.notes_cursor);
                }
            }
            KeyCode::Delete => {
                if app.notes_cursor < app.notes_buffer.len() {
                    app.notes_buffer.remove(app.notes_cursor);
                }
            }
            KeyCode::Left => {
                if app.notes_cursor > 0 {
                    app.notes_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if app.notes_cursor < app.notes_buffer.len() {
                    app.notes_cursor += 1;
                }
            }
            KeyCode::Up => {
                // Move cursor up one line
                let before_cursor = &app.notes_buffer[..app.notes_cursor];
                if let Some(prev_newline) = before_cursor.rfind('\n') {
                    let current_line_start = prev_newline + 1;
                    let col = app.notes_cursor - current_line_start;

                    if prev_newline > 0 {
                        let before_prev = &app.notes_buffer[..prev_newline];
                        let prev_line_start = before_prev.rfind('\n').map(|p| p + 1).unwrap_or(0);
                        let prev_line_len = prev_newline - prev_line_start;
                        app.notes_cursor = prev_line_start + col.min(prev_line_len);
                    } else {
                        app.notes_cursor = col.min(prev_newline);
                    }
                }
            }
            KeyCode::Down => {
                // Move cursor down one line
                let after_cursor = &app.notes_buffer[app.notes_cursor..];
                if let Some(next_newline_rel) = after_cursor.find('\n') {
                    let current_line_start = app.notes_buffer[..app.notes_cursor]
                        .rfind('\n')
                        .map(|p| p + 1)
                        .unwrap_or(0);
                    let col = app.notes_cursor - current_line_start;
                    let next_line_start = app.notes_cursor + next_newline_rel + 1;

                    if next_line_start < app.notes_buffer.len() {
                        let remaining = &app.notes_buffer[next_line_start..];
                        let next_line_len = remaining.find('\n').unwrap_or(remaining.len());
                        app.notes_cursor = next_line_start + col.min(next_line_len);
                    }
                }
            }
            KeyCode::Home => {
                // Move to start of line
                let before_cursor = &app.notes_buffer[..app.notes_cursor];
                app.notes_cursor = before_cursor.rfind('\n').map(|p| p + 1).unwrap_or(0);
            }
            KeyCode::End => {
                // Move to end of line
                let after_cursor = &app.notes_buffer[app.notes_cursor..];
                if let Some(next_newline) = after_cursor.find('\n') {
                    app.notes_cursor += next_newline;
                } else {
                    app.notes_cursor = app.notes_buffer.len();
                }
            }
            _ => {}
        }
    } else if app.input_mode {
        match key.code {
            KeyCode::Enter => app.add_task(),
            KeyCode::Esc => {
                app.input_mode = false;
                app.time_input_field = 0;
                app.editing_task_idx = None;
                app.input_buffer.clear();
                app.start_time_buffer.clear();
                app.end_time_buffer.clear();
            }
            KeyCode::Tab => {
                app.time_input_field = (app.time_input_field + 1) % 3;
            }
            KeyCode::Char(c) => {
                match app.time_input_field {
                    0 => app.input_buffer.push(c),
                    1 => {
                        if app.start_time_buffer.len() < 5 && (c.is_ascii_digit() || c == ':') {
                            app.start_time_buffer.push(c);
                        }
                    }
                    2 => {
                        if app.end_time_buffer.len() < 5 && (c.is_ascii_digit() || c == ':') {
                            app.end_time_buffer.push(c);
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                match app.time_input_field {
                    0 => { app.input_buffer.pop(); }
                    1 => { app.start_time_buffer.pop(); }
                    2 => { app.end_time_buffer.pop(); }
                    _ => {}
                }
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('n') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.input_mode = true;
                }
            }
            KeyCode::Char('e') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.start_edit_task();
                }
            }
            KeyCode::Char(' ') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.toggle_task();
                }
            }
            KeyCode::Char('d') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.delete_task();
                }
            }
            KeyCode::Tab => app.toggle_view(),
            KeyCode::Up | KeyCode::Char('k') => app.prev_task(),
            KeyCode::Down | KeyCode::Char('j') => app.next_task(),
            KeyCode::Left | KeyCode::Char('h') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.prev_day();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if app.view_mode == ViewMode::Scheduled {
                    app.next_day();
                }
            }
            _ => {}
        }
    }
    Ok(())
}
