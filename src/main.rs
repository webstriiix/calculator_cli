use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
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

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_key_events(key),
            _ => {}
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) {
        if self.error_message.is_some() {
            match key.code {
                KeyCode::Char('a') | KeyCode::Char('A') => self.all_clear(),
                KeyCode::Char('q') => self.exit = true,
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char('a') | KeyCode::Char('A') => self.all_clear(),
            KeyCode::Enter | KeyCode::Char('=') => self.evaluate(),
            KeyCode::Char('+') => self.set_operator(Operator::Add),
            KeyCode::Char('-') => self.set_operator(Operator::Subtract),
            KeyCode::Char('*') | KeyCode::Char('x') | KeyCode::Char('X') => {
                self.set_operator(Operator::Multiply)
            }
            KeyCode::Char('/') | KeyCode::Char(':') => self.set_operator(Operator::Divide),
            KeyCode::Char('.') => self.handle_decimal_point(),
            KeyCode::Backspace => self.handle_backspace(),
            KeyCode::Char(ch) if ch.is_ascii_digit() => self.handle_digit(ch),
            _ => {}
        }
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
            return format!("{err} (press A to clear)");
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
            "Enter digits and choose an operator".into()
        } else {
            parts.join(" ")
        }
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

        let expression = Paragraph::new(self.expression_line())
            .block(Block::bordered().title("Expression"))
            .alignment(ratatui::layout::Alignment::Right);

        let value = Paragraph::new(Span::styled(
            self.display_value(),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::bordered().title("Result"));

        let instruction = Paragraph::new(Line::from(vec![
            Span::styled("Digits 0-9", Style::default().add_modifier(Modifier::BOLD)),
            "· + - * : ".into(),
            "· Enter/=: evaluate ".into(),
            "· A: AC ".into(),
            "· Q: Quit".into(),
        ]))
        .block(Block::bordered());

        expression.render(layout[0], buf);
        value.render(layout[1], buf);
        instruction.render(layout[2], buf);
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
    fn render_shows_expression_result_and_instructions() {
        let app = App::default();
        let area = Rect::new(0, 0, 60, 9);
        let mut buf = Buffer::empty(area);

        (&app).render(area, &mut buf);

        assert!(row_string(&buf, 1, area.width).contains("Enter digits"));
        assert!(row_string(&buf, 4, area.width).contains("0"));
        assert!(row_string(&buf, 7, area.width).contains("Digits 0-9"));
    }

    fn row_string(buf: &Buffer, row: u16, width: u16) -> String {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, row)].symbol());
        }
        line
    }
}
