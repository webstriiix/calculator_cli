use std::io;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
    let app_result = App::default().run(&mut terminal);
    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    app_result
}

/// Stateful calculator application.
///
/// Inspired by the “deep module” principle from Ousterhout’s *A Philosophy of
/// Software Design*, `App` keeps the entire calculator state (current input,
/// committed tokens, error handling, and event-driven behavior) behind a single
/// interface so the rest of the program interacts with a clear abstraction
/// boundary.
#[derive(Debug, Default, Clone)]
pub struct App {
    input: String,
    tokens: Vec<Token>,
    just_evaluated: bool,
    error_message: Option<String>,
    exit: bool,
    buttons: Vec<Button>,
}

#[derive(Debug, Clone)]
enum Token {
    Number(String),
    Operator(Operator),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Copy)]
enum ButtonAction {
    Digit(char),
    Decimal,
    Operator(Operator),
    Evaluate,
    AllClear,
    Backspace,
    Quit,
    NoOp,
}

#[derive(Debug, Clone, Copy)]
struct Button {
    action: ButtonAction,
    area: Rect,
}

impl Operator {
    fn symbol(self) -> char {
        match self {
            Operator::Add => '+',
            Operator::Subtract => '-',
            Operator::Multiply => '×',
            Operator::Divide => '÷',
        }
    }
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key_events(key),
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            _ => {}
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) {
        let action = match key.code {
            KeyCode::Char('q') => Some(ButtonAction::Quit),
            KeyCode::Char('a') | KeyCode::Char('A') => Some(ButtonAction::AllClear),
            KeyCode::Enter | KeyCode::Char('=') => Some(ButtonAction::Evaluate),
            KeyCode::Char('+') => Some(ButtonAction::Operator(Operator::Add)),
            KeyCode::Char('-') => Some(ButtonAction::Operator(Operator::Subtract)),
            KeyCode::Char('*') | KeyCode::Char('x') | KeyCode::Char('X') => {
                Some(ButtonAction::Operator(Operator::Multiply))
            }
            KeyCode::Char('/') | KeyCode::Char(':') => {
                Some(ButtonAction::Operator(Operator::Divide))
            }
            KeyCode::Char('.') => Some(ButtonAction::Decimal),
            KeyCode::Backspace => Some(ButtonAction::Backspace),
            KeyCode::Char(ch) if ch.is_ascii_digit() => Some(ButtonAction::Digit(ch)),
            _ => None,
        };

        if let Some(action) = action {
            self.handle_action(action);
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if let Some(action) = self.hit_test(mouse.column, mouse.row) {
                self.handle_action(action);
            }
        }
    }

    fn handle_action(&mut self, action: ButtonAction) {
        if self.error_message.is_some() {
            match action {
                ButtonAction::AllClear => self.all_clear(),
                ButtonAction::Quit => self.exit = true,
                _ => {}
            }
            return;
        }

        match action {
            ButtonAction::Digit(ch) => self.handle_digit(ch),
            ButtonAction::Decimal => self.handle_decimal_point(),
            ButtonAction::Operator(op) => self.set_operator(op),
            ButtonAction::Evaluate => self.evaluate(),
            ButtonAction::AllClear => self.all_clear(),
            ButtonAction::Backspace => self.handle_backspace(),
            ButtonAction::Quit => self.exit = true,
            ButtonAction::NoOp => {}
        }
    }

    fn hit_test(&self, column: u16, row: u16) -> Option<ButtonAction> {
        self.buttons.iter().find_map(|button| {
            let area = button.area;
            let within_x = column >= area.x && column < area.x.saturating_add(area.width);
            let within_y = row >= area.y && row < area.y.saturating_add(area.height);
            if within_x && within_y {
                Some(button.action)
            } else {
                None
            }
        })
    }

    fn all_clear(&mut self) {
        self.input.clear();
        self.tokens.clear();
        self.error_message = None;
        self.just_evaluated = false;
    }

    fn handle_digit(&mut self, digit: char) {
        if self.just_evaluated {
            self.input.clear();
            self.just_evaluated = false;
        }

        if self.input == "0" {
            self.input.clear();
        }

        self.input.push(digit);
    }

    fn handle_decimal_point(&mut self) {
        if self.just_evaluated {
            self.input.clear();
            self.just_evaluated = false;
        }

        if self.input.is_empty() {
            self.input.push('0');
        }
        if !self.input.contains('.') {
            self.input.push('.');
        }
    }

    fn handle_backspace(&mut self) {
        if self.just_evaluated || self.input.is_empty() {
            return;
        }
        self.input.pop();
    }

    fn set_operator(&mut self, operator: Operator) {
        if !self.try_commit_input() {
            return;
        }

        if self.tokens.is_empty() {
            // no operand to attach the operator to
            return;
        }

        match self.tokens.last_mut() {
            Some(Token::Operator(current)) => *current = operator,
            _ => self.tokens.push(Token::Operator(operator)),
        }
        self.just_evaluated = false;
    }

    fn evaluate(&mut self) {
        if !self.try_commit_input() {
            return;
        }
        if let Some(Token::Operator(_)) = self.tokens.last() {
            // trailing operator means expression is incomplete
            return;
        }
        if self.tokens.is_empty() {
            return;
        }

        match self.evaluate_tokens() {
            Ok(result) => {
                self.input = self.format_number(result);
                self.tokens.clear();
                self.just_evaluated = true;
            }
            Err(msg) => self.set_error(msg),
        }
    }

    fn evaluate_tokens(&self) -> Result<f64, &'static str> {
        let mut values = Vec::new();
        let mut operators = Vec::new();
        let mut expect_number = true;

        for token in &self.tokens {
            match token {
                Token::Number(text) => {
                    if !expect_number {
                        return Err("invalid expression");
                    }
                    let value = text
                        .parse::<f64>()
                        .map_err(|_| "invalid number in expression")?;
                    values.push(value);
                    expect_number = false;
                }
                Token::Operator(op) => {
                    if expect_number {
                        return Err("incomplete expression");
                    }
                    operators.push(*op);
                    expect_number = true;
                }
            }
        }

        if values.is_empty() {
            return Err("incomplete expression");
        }

        let mut values = values;
        let mut operators = operators;

        let mut idx = 0;
        while idx < operators.len() {
            match operators[idx] {
                Operator::Multiply | Operator::Divide => {
                    let lhs = values[idx];
                    let rhs = values[idx + 1];
                    let result = self.apply_operator(lhs, rhs, operators[idx])?;
                    values[idx] = result;
                    values.remove(idx + 1);
                    operators.remove(idx);
                }
                _ => idx += 1,
            }
        }

        let mut result = values[0];
        for (op, rhs) in operators.into_iter().zip(values.into_iter().skip(1)) {
            result = self.apply_operator(result, rhs, op)?;
        }
        Ok(result)
    }

    fn try_commit_input(&mut self) -> bool {
        if self.input.is_empty() {
            return true;
        }

        match self.input.parse::<f64>() {
            Ok(_) => {
                self.tokens.push(Token::Number(self.input.clone()));
                self.input.clear();
                self.just_evaluated = false;
                true
            }
            Err(_) => {
                self.set_error("invalid number");
                false
            }
        }
    }

    fn apply_operator(&self, lhs: f64, rhs: f64, operator: Operator) -> Result<f64, &'static str> {
        match operator {
            Operator::Add => Ok(lhs + rhs),
            Operator::Subtract => Ok(lhs - rhs),
            Operator::Multiply => Ok(lhs * rhs),
            Operator::Divide => {
                if rhs.abs() < f64::EPSILON {
                    Err("Cannot divide by zero")
                } else {
                    Ok(lhs / rhs)
                }
            }
        }
    }

    fn set_error(&mut self, message: &'static str) {
        self.error_message = Some(format!("Error {}", message));
        self.input.clear();
        self.tokens.clear();
        self.just_evaluated = false;
    }

    fn format_number(&self, value: f64) -> String {
        let mut output = format!("{}", value);
        if output.contains('.') {
            while output.ends_with('0') {
                output.pop();
            }
            if output.ends_with('.') {
                output.pop();
            }
        }
        if output.is_empty() {
            "0".into()
        } else {
            output
        }
    }

    fn display_value(&self) -> String {
        if let Some(err) = &self.error_message {
            return err.clone();
        }
        if !self.input.is_empty() {
            return self.input.clone();
        }
        if let Some(value) = self.tokens.iter().rev().find_map(|token| match token {
            Token::Number(number) => Some(number.clone()),
            Token::Operator(_) => None,
        }) {
            return value;
        }
        "0".into()
    }

    fn expression_line(&self) -> String {
        if let Some(err) = &self.error_message {
            return format!("{err} (press A or click AC to clear)");
        }

        let mut parts: Vec<String> = self
            .tokens
            .iter()
            .map(|token| match token {
                Token::Number(number) => number.clone(),
                Token::Operator(op) => op.symbol().to_string(),
            })
            .collect();
        if !self.input.is_empty() {
            parts.push(self.input.clone());
        }

        if parts.is_empty() {
            "Click buttons or use the keyboard".into()
        } else {
            parts.join(" ")
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        let expression = Paragraph::new(self.expression_line())
            .block(Block::bordered().title("Expression"))
            .alignment(Alignment::Right);

        let value = Paragraph::new(Span::styled(
            self.display_value(),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Right)
        .block(Block::bordered().title("Result"));

        expression.render(layout[0], buf);
        value.render(layout[1], buf);
        self.render_buttons(layout[2], buf);
    }
}

impl App {
    fn render_buttons(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();

        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

        let grid: [[(&'static str, ButtonAction); 4]; 5] = [
            [
                ("AC", ButtonAction::AllClear),
                ("DEL", ButtonAction::Backspace),
                ("", ButtonAction::NoOp),
                ("", ButtonAction::NoOp),
            ],
            [
                ("7", ButtonAction::Digit('7')),
                ("8", ButtonAction::Digit('8')),
                ("9", ButtonAction::Digit('9')),
                ("÷", ButtonAction::Operator(Operator::Divide)),
            ],
            [
                ("4", ButtonAction::Digit('4')),
                ("5", ButtonAction::Digit('5')),
                ("6", ButtonAction::Digit('6')),
                ("×", ButtonAction::Operator(Operator::Multiply)),
            ],
            [
                ("1", ButtonAction::Digit('1')),
                ("2", ButtonAction::Digit('2')),
                ("3", ButtonAction::Digit('3')),
                ("-", ButtonAction::Operator(Operator::Subtract)),
            ],
            [
                ("0", ButtonAction::Digit('0')),
                (".", ButtonAction::Decimal),
                ("=", ButtonAction::Evaluate),
                ("+", ButtonAction::Operator(Operator::Add)),
            ],
        ];

        for (row_area, row) in rows.iter().zip(grid) {
            let cols = Layout::horizontal([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(*row_area);

            for (area, (label, action)) in cols.iter().zip(row) {
                if label.is_empty() {
                    continue;
                }
                let area = *area;
                self.buttons.push(Button { action, area });
                let label_style = match action {
                    ButtonAction::Operator(_) | ButtonAction::Evaluate => {
                        Style::default().add_modifier(Modifier::BOLD)
                    }
                    ButtonAction::AllClear | ButtonAction::Backspace | ButtonAction::Quit => {
                        Style::default().add_modifier(Modifier::DIM)
                    }
                    ButtonAction::NoOp => Style::default().add_modifier(Modifier::DIM),
                    _ => Style::default(),
                };
                let block_style = match action {
                    ButtonAction::Operator(_) => Style::default().fg(Color::Blue),
                    _ => Style::default(),
                };
                Paragraph::new(Line::from(vec![Span::styled(label, label_style)]))
                    .alignment(Alignment::Center)
                    .block(Block::bordered().style(block_style))
                    .render(area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn digit_entry_and_decimal_behavior() {
        let mut app = App::default();
        app.handle_digit('0');
        app.handle_digit('5');
        assert_eq!(app.input, "5");

        app.handle_decimal_point();
        app.handle_digit('2');
        assert_eq!(app.input, "5.2");

        app.set_operator(Operator::Add);
        app.handle_digit('1');
        app.evaluate();
        assert_eq!(app.display_value(), "6.2");
        assert!(app.just_evaluated);

        app.handle_digit('3');
        assert_eq!(app.input, "3");
    }

    #[test]
    fn backspace_removes_last_digit() {
        let mut app = App::default();
        app.handle_digit('2');
        app.handle_digit('0');
        app.handle_digit('0');
        app.handle_digit('0');

        app.handle_backspace();
        app.handle_backspace();
        assert_eq!(app.input, "20");

        app.set_operator(Operator::Add);
        app.handle_digit('1');
        app.evaluate();
        assert_eq!(app.display_value(), "21");
    }

    #[test]
    fn full_expression_respects_precedence() {
        let mut app = App::default();
        for ch in "10".chars() {
            app.handle_digit(ch);
        }
        app.set_operator(Operator::Add);

        for ch in "10".chars() {
            app.handle_digit(ch);
        }
        app.set_operator(Operator::Multiply);
        app.handle_digit('5');

        app.set_operator(Operator::Divide);
        app.handle_digit('4');

        app.set_operator(Operator::Add);
        for ch in "45".chars() {
            app.handle_digit(ch);
        }

        app.evaluate();
        assert_eq!(app.display_value(), "67.5");
        assert!(app.tokens.is_empty());
    }

    #[test]
    fn divide_by_zero_sets_error() {
        let mut app = App::default();
        app.handle_digit('8');
        app.set_operator(Operator::Divide);
        app.handle_digit('0');
        app.evaluate();

        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|msg| msg.contains("Cannot divide"))
        );
    }

    #[test]
    fn all_clear_resets_state() {
        let mut app = App::default();
        app.handle_digit('9');
        app.set_operator(Operator::Subtract);
        app.handle_digit('4');
        app.evaluate();
        assert!(app.just_evaluated);

        app.all_clear();
        assert!(app.input.is_empty());
        assert!(app.tokens.is_empty());
        assert!(app.error_message.is_none());
        assert!(!app.just_evaluated);
    }

    #[test]
    fn render_shows_expression_result_and_buttons() {
        let mut app = App::default();
        let area = Rect::new(0, 0, 60, 21);
        let mut buf = Buffer::empty(area);

        (&mut app).render(area, &mut buf);

        let all = buffer_string(&buf, area);
        assert!(all.contains("Expression"));
        assert!(all.contains("Result"));
        assert!(all.contains("AC"));
        assert!(all.contains("7"));
    }

    fn row_string(buf: &Buffer, row: u16, width: u16) -> String {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, row)].symbol());
        }
        line
    }

    fn buffer_string(buf: &Buffer, area: Rect) -> String {
        let mut content = String::new();
        for row in 0..area.height {
            content.push_str(&row_string(buf, row, area.width));
        }
        content
    }
}
