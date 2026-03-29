// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::str::FromStr;

#[derive(Clone, Debug)]
enum Statement {
    Print(Vec<String>),
    Goto(usize),
    End,
    Rem,
    Pause,
}

#[derive(Clone, Debug)]
struct ProgramLine {
    number: usize,
    statement: Statement,
}

pub fn run_basic_from_document(state: &crate::State) {
    if let Some(doc) = state.documents.active() {
        let tb = doc.buffer.borrow();
        let source_bytes = tb.read_forward(0);
        let source = String::from_utf8_lossy(source_bytes);
        run_basic(&source);
    }
}

pub fn run_basic(source: &str) {
    let mut tty_out = match File::options().write(true).open("/dev/tty") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Cannot open /dev/tty: {}", e);
            return;
        }
    };

    let mut tty_in = match File::options().read(true).open("/dev/tty") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Cannot open /dev/tty: {}", e);
            return;
        }
    };

    tty_out.write_all(b"\x1b[?1049l\x1b[0 q\x1b[?25h\x1b[2J\x1b[H").ok();
    tty_out.flush().ok();

    restore_terminal_raw_mode(tty_in.as_raw_fd());

    match parse_program(source) {
        Ok(program) => {
            if let Err(e) = execute_program(&program, &mut tty_out, &mut tty_in) {
                let _ = writeln!(tty_out, "\r\nError: {}", e);
            }
        }
        Err(e) => {
            let _ = writeln!(tty_out, "\r\nError: {}", e);
        }
    }

    tty_out.write_all(b"\r\nPress any key to return to the editor...\x1b[0m").ok();
    tty_out.flush().ok();

    let mut buf = [0u8];
    let _ = tty_in.read(&mut buf);

    // Flush any pending input (e.g. if the user pressed an arrow key which generated multiple bytes)
    unsafe {
        libc::tcflush(tty_in.as_raw_fd(), libc::TCIFLUSH);
    }

    // Re-enter alt buffer and restore all modes that setup_terminal enabled.
    // 1049: Alternative Screen Buffer
    // 1002: Cell Motion Mouse Tracking
    // 1006: SGR Mouse Mode
    // 2004: Bracketed Paste Mode
    // 1036: Xterm: "meta sends escape"
    tty_out.write_all(b"\x1b[?1049h\x1b[?1002;1006;2004h\x1b[?1036h\x1b[2J\x1b[H\x1b[3J").ok();
    tty_out.flush().ok();

    // Re-initialize sys to ensure stdin/stdout state is correct
    let _ = edit::sys::switch_modes();
}

fn restore_terminal_raw_mode(fd: std::os::unix::io::RawFd) {
    use libc::{ECHO, ICANON, TCSANOW, tcflag_t, tcgetattr, tcsetattr, termios};

    let mut term: termios = unsafe { std::mem::zeroed() };

    if unsafe { tcgetattr(fd, &mut term) } != 0 {
        return;
    }

    term.c_lflag &= !(ECHO as tcflag_t | ICANON as tcflag_t);

    let _ = unsafe { tcsetattr(fd, TCSANOW, &term) };
}

fn parse_program(source: &str) -> Result<Vec<ProgramLine>, String> {
    let mut program = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let line = match line.find(|c: char| !c.is_ascii_digit() && c != ' ') {
            Some(0) => return Err(format!("Line must start with line number: {}", line)),
            Some(pos) => {
                let (num_str, rest) = line.split_at(pos);
                let number = usize::from_str(num_str.trim())
                    .map_err(|_| format!("Invalid line number: {}", num_str.trim()))?;
                (number, rest.trim())
            }
            None => {
                let _ = usize::from_str(line.trim())
                    .map_err(|_| format!("Invalid line number: {}", line))?;
                continue;
            }
        };

        let (number, rest) = line;
        if rest.is_empty() {
            continue;
        }

        let statement = parse_statement(rest)?;
        program.push(ProgramLine { number, statement });
    }

    program.sort_by_key(|l| l.number);
    Ok(program)
}

fn parse_statement(rest: &str) -> Result<Statement, String> {
    let upper = rest.to_uppercase();
    let upper = upper.as_str();

    if upper.starts_with("PRINT ") || upper == "PRINT" {
        let args = if upper == "PRINT" {
            vec![]
        } else {
            let content = &rest[6..].trim();
            parse_print_args(content)?
        };
        Ok(Statement::Print(args))
    } else if upper.starts_with("GOTO ") {
        let target = rest[5..].trim();
        let target_num =
            usize::from_str(target).map_err(|_| format!("Invalid GOTO target: {}", target))?;
        Ok(Statement::Goto(target_num))
    } else if upper == "END" {
        Ok(Statement::End)
    } else if upper == "PAUSE" {
        Ok(Statement::Pause)
    } else if upper.starts_with("REM ") || upper == "REM" {
        Ok(Statement::Rem)
    } else if upper.starts_with("'") {
        Ok(Statement::Rem)
    } else {
        Err(format!("Unknown statement: {}", rest))
    }
}

fn parse_print_args(content: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            in_quotes = !in_quotes;
            current.push(c);
        } else if c == ';' && !in_quotes {
            args.push(current.trim().to_string());
            current = String::new();
        } else if c == ',' && !in_quotes {
            args.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }

    if !current.trim().is_empty() || args.is_empty() {
        args.push(current.trim().to_string());
    }

    Ok(args)
}

fn execute_program<W: Write, R: Read>(
    program: &[ProgramLine],
    tty_out: &mut W,
    tty_in: &mut R,
) -> Result<(), String> {
    let mut pc = 0;
    let mut loop_count = 0;
    let max_iterations = 100000;

    while pc < program.len() {
        loop_count += 1;
        if loop_count > max_iterations {
            return Err("Program exceeded maximum iterations (possible infinite loop)".to_string());
        }

        let ProgramLine { number: _, statement } = &program[pc];

        match statement {
            Statement::Print(args) => {
                for arg in args {
                    let arg = arg.trim();
                    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                        let text = &arg[1..arg.len() - 1];
                        write!(tty_out, "{}", text).map_err(|e| e.to_string())?;
                    } else if !arg.is_empty() {
                        let evaluated = eval_expression(arg)?;
                        write!(tty_out, "{}", evaluated).map_err(|e| e.to_string())?;
                    }
                }
                write!(tty_out, "\r\n").map_err(|e| e.to_string())?;
                tty_out.flush().map_err(|e| e.to_string())?;
                pc += 1;
            }
            Statement::Goto(target) => {
                let new_pc = program.iter().position(|l| l.number == *target);
                match new_pc {
                    Some(idx) => pc = idx,
                    None => return Err(format!("GOTO {}: line not found", target)),
                }
            }
            Statement::End => {
                break;
            }
            Statement::Pause => {
                write!(tty_out, "\r\nPress any key to continue...\r\n")
                    .map_err(|e| e.to_string())?;
                tty_out.flush().map_err(|e| e.to_string())?;
                wait_for_key_raw(tty_in);
                break;
            }
            Statement::Rem => {
                pc += 1;
            }
        }
    }

    Ok(())
}

fn eval_expression(expr: &str) -> Result<String, String> {
    let expr = expr.trim();

    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('"') && !expr.contains('"'))
    {
        return Err("Unterminated string".to_string());
    }

    if expr.starts_with('"') && expr.ends_with('"') {
        return Ok(expr[1..expr.len() - 1].to_string());
    }

    let no_spaces = expr.replace(" ", "");
    if let Ok(val) = eval_math_expr(&no_spaces) {
        if val.fract() == 0.0 {
            return Ok(format!("{:.0}", val));
        }
        return Ok(val.to_string());
    }

    if let Some(plus_pos) = expr.find('+') {
        if plus_pos > 0 && plus_pos < expr.len() - 1 {
            let left = eval_expression(&expr[..plus_pos])?;
            let right = eval_expression(&expr[plus_pos + 1..])?;
            return Ok(format!("{}{}", left, right));
        }
    }

    if let Ok(num) = i64::from_str(expr) {
        return Ok(num.to_string());
    }
    if let Ok(num) = f64::from_str(expr) {
        if num.fract() == 0.0 {
            return Ok(format!("{:.0}", num));
        }
        return Ok(num.to_string());
    }

    Ok(expr.to_string())
}

fn eval_math_expr(expr: &str) -> Result<f64, String> {
    eval_math_add_sub(expr)
}

fn eval_math_add_sub(expr: &str) -> Result<f64, String> {
    let mut depth = 0;
    for (i, c) in expr.char_indices().rev() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        } else if depth == 0 && (c == '+' || c == '-') && i > 0 {
            let prev_char = expr[..i].chars().last().unwrap();
            if prev_char == '+' || prev_char == '-' || prev_char == '*' || prev_char == '/' {
                continue;
            }
            let left = eval_math_add_sub(&expr[..i])?;
            let right = eval_math_mul_div(&expr[i + 1..])?;
            if c == '+' {
                return Ok(left + right);
            } else {
                return Ok(left - right);
            }
        }
    }
    eval_math_mul_div(expr)
}

fn eval_math_mul_div(expr: &str) -> Result<f64, String> {
    let mut depth = 0;
    for (i, c) in expr.char_indices().rev() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        } else if depth == 0 && (c == '*' || c == '/') && i > 0 {
            let left = eval_math_mul_div(&expr[..i])?;
            let right = eval_math_value(&expr[i + 1..])?;
            if c == '*' {
                return Ok(left * right);
            } else {
                if right == 0.0 {
                    return Err("Division by zero".to_string());
                }
                return Ok(left / right);
            }
        }
    }
    eval_math_value(expr)
}

fn eval_math_value(expr: &str) -> Result<f64, String> {
    let expr = expr.trim();
    if expr.starts_with('(') && expr.ends_with(')') {
        return eval_math_add_sub(&expr[1..expr.len() - 1]);
    }
    f64::from_str(expr).map_err(|_| format!("Invalid number: {}", expr))
}

fn wait_for_key_raw<R: Read>(tty_in: &mut R) {
    let mut buf = [0u8];
    let _ = tty_in.read(&mut buf);
}
