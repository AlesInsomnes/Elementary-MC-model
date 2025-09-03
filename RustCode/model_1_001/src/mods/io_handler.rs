use crate::mods::{
    constants::{
        COMMENT_LINE, CONFIG_FILE_NAME, INIT_TIME_STATES_FILE_NAME, TIME_STATES_FILE_NAME,
    },
    settings::{Settings, SettingsError},
};
use chrono::Utc;
use std::{
    collections::HashMap,
    env::current_exe,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Error as IoError, ErrorKind, Result as IoResult, Write},
    path::PathBuf,
};

use evalexpr::{eval_boolean, eval_number};

macro_rules! parse_and_assign_eval {
    ($map:expr, $field:ident, $type:ty, $key:expr, boolean) => {
        $map.insert(
            $key,
            Box::new(|v: &str, s: &mut Settings| {
                let val = eval_boolean(v).map_err(|e| SettingsError::new($key, v, e))?;
                s.$field = val as $type;
                Ok(())
            }),
        );
    };

    ($map:expr, $field:ident, $type:ty, $key:expr, number) => {
        $map.insert(
            $key,
            Box::new(|v: &str, s: &mut Settings| {
                let val = eval_number(v).map_err(|e| SettingsError::new($key, v, e))?;
                s.$field = val as $type;
                Ok(())
            }),
        );
    };
}

pub fn load_config(cfg: &mut Settings) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(cfg.src_path.join(CONFIG_FILE_NAME))?;
    let reader = BufReader::new(file);

    let mut dispatch: HashMap<&str, Box<dyn Fn(&str, &mut Settings) -> Result<(), SettingsError>>> =
        HashMap::new();

    dispatch.insert(
        "DirPrefix",
        Box::new(|v, s| {
            s.dir_prefix = v.to_string();
            Ok(())
        }),
    );
    parse_and_assign_eval!(dispatch, seed, u64, "Seed", number);

    parse_and_assign_eval!(dispatch, sx, usize, "Sx", number);
    parse_and_assign_eval!(dispatch, sy, usize, "Sy", number);
    parse_and_assign_eval!(dispatch, sz, usize, "Sz", number);
    parse_and_assign_eval!(dispatch, px, bool, "Px", boolean);
    parse_and_assign_eval!(dispatch, py, bool, "Py", boolean);
    parse_and_assign_eval!(dispatch, pz, bool, "Pz", boolean);

    parse_and_assign_eval!(dispatch, temperature, f64, "T", number);
    parse_and_assign_eval!(dispatch, ax, f64, "Ax", number);
    parse_and_assign_eval!(dispatch, ay, f64, "Ay", number);
    parse_and_assign_eval!(dispatch, az, f64, "Az", number);

    parse_and_assign_eval!(dispatch, g100, f64, "g100", number);
    parse_and_assign_eval!(dispatch, g010, f64, "g010", number);
    parse_and_assign_eval!(dispatch, g001, f64, "g001", number);

    parse_and_assign_eval!(dispatch, mode, f64, "mode", number);
    parse_and_assign_eval!(dispatch, dg, f64, "dg", number);
    parse_and_assign_eval!(dispatch, c_eq, f64, "C_eq", number);
    parse_and_assign_eval!(dispatch, c0, f64, "C0", number);
    parse_and_assign_eval!(dispatch, n_tot, f64, "N_tot", number);
    parse_and_assign_eval!(dispatch, n0_cr, f64, "N0_cr", number);
    parse_and_assign_eval!(dispatch, p_b, f64, "p_b", number);
    parse_and_assign_eval!(dispatch, p_pow, f64, "p_pow", number);

    parse_and_assign_eval!(dispatch, add_i, u64, "AddI", number);
    parse_and_assign_eval!(dispatch, add_from, u64, "AddFrom", number);
    parse_and_assign_eval!(dispatch, rem_i, u64, "RemI", number);
    parse_and_assign_eval!(dispatch, rem_from, u64, "RemFrom", number);

    parse_and_assign_eval!(dispatch, load_prev, i64, "LoadPrev", number);

    parse_and_assign_eval!(dispatch, step_lim, u64, "StepLim", number);
    parse_and_assign_eval!(dispatch, print_i, u64, "PrintI", number);
    parse_and_assign_eval!(dispatch, write_i, u64, "WriteI", number);

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let trimmed = line.trim();

        if trimmed == COMMENT_LINE {
            break;
        }

        let mut parts = trimmed.splitn(2, ':');
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();

        if key.is_empty() || value.is_empty() {
            #[cfg(debug_assertions)]
            eprintln!("⚠️ Warning: Malformed line {}: '{}'", line_num + 1, line);
            continue;
        }

        if let Some(parser) = dispatch.get(key) {
            parser(value, cfg)?;
        } else {
            #[cfg(debug_assertions)]
            eprintln!(
                "⚠️ Warning: Unknown cfg key '{}' found on line {}: '{}'",
                key,
                line_num + 1,
                line
            );
        }
    }

    Ok(())
}

pub fn get_exe_dir() -> IoResult<PathBuf> {
    current_exe()
        .map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!("Failed to get executable path: {}", e),
            )
        })?
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| IoError::new(ErrorKind::Other, "Failed to get executable directory"))
}

fn create_dir_name(cfg: &Settings, timestamp: i64) -> String {
    match cfg.mode {
        1.1 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_dg{:e}",
            timestamp, cfg.dir_prefix, cfg.sx, cfg.sy, cfg.sz, cfg.temperature, cfg.dg
        ),
        1.2 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_dg{:e}_Pb{:?}",
            timestamp, cfg.dir_prefix, cfg.sx, cfg.sy, cfg.sz, cfg.temperature, cfg.dg, cfg.p_b
        ),
        1.3 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_dg{:e}_Pb{:?}_Pp{:?}",
            timestamp,
            cfg.dir_prefix,
            cfg.sx,
            cfg.sy,
            cfg.sz,
            cfg.temperature,
            cfg.dg,
            cfg.p_b,
            cfg.p_pow
        ),
        2.1 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_C{:e}_Nt{:e}",
            timestamp, cfg.dir_prefix, cfg.sx, cfg.sy, cfg.sz, cfg.temperature, cfg.c0, cfg.n_tot
        ),
        2.2 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_C{:e}_Nt{:e}_Pb{:?}",
            timestamp,
            cfg.dir_prefix,
            cfg.sx,
            cfg.sy,
            cfg.sz,
            cfg.temperature,
            cfg.c0,
            cfg.n_tot,
            cfg.p_b
        ),
        2.3 => format!(
            "{}_{}_X{}Y{}Z{}_T{:e}_C{:e}_Nt{:e}_Pb{:?}_Pp{:?}",
            timestamp,
            cfg.dir_prefix,
            cfg.sx,
            cfg.sy,
            cfg.sz,
            cfg.temperature,
            cfg.c0,
            cfg.n_tot,
            cfg.p_b,
            cfg.p_pow
        ),
        _ => format!(
            "{}_{}_X{}Y{}Z{}",
            timestamp, cfg.dir_prefix, cfg.sx, cfg.sy, cfg.sz
        ),
    }
}

pub fn prepare_dir(cfg: &mut Settings) -> IoResult<()> {
    let timestamp = Utc::now().timestamp_micros();
    let dir_name = create_dir_name(cfg, timestamp);
    let res_dir = cfg.src_path.join(&dir_name);

    fs::create_dir_all(&res_dir).map_err(|e| {
        IoError::new(
            ErrorKind::Other,
            format!("Failed to create directory '{}': {}", res_dir.display(), e),
        )
    })?;

    cfg.dst_path = res_dir;

    Ok(())
}

pub fn prepare_files(cfg: &mut Settings) -> IoResult<PathBuf> {
    let path_src_config = cfg.src_path.join(CONFIG_FILE_NAME);
    let path_dst_config = cfg.dst_path.join(CONFIG_FILE_NAME);

    if path_src_config.exists() {
        fs::copy(&path_src_config, &path_dst_config).map_err(|e| {
            IoError::new(
                ErrorKind::Other,
                format!(
                    "Failed to copy configuration file from '{}' to '{}': {}",
                    path_src_config.display(),
                    path_dst_config.display(),
                    e
                ),
            )
        })?;
    } else {
        eprintln!(
            "⚠️ Warning: Configuration file '{}' not found next to the binary.",
            path_src_config.display()
        );
    }

    let path_dst_states = cfg.dst_path.join(TIME_STATES_FILE_NAME);

    File::create(&path_dst_states).map_err(|e| {
        IoError::new(
            ErrorKind::Other,
            format!(
                "Failed to create file '{}': {}",
                path_dst_states.display(),
                e
            ),
        )
    })?;

    Ok(path_dst_states)
}

pub fn load_state(states: &mut Box<[u8]>, cfg: &Settings) -> IoResult<()> {
    let load_line = cfg.load_prev;
    if load_line == 0 {
        return Ok(());
    }

    let file = File::open(cfg.src_path.join(INIT_TIME_STATES_FILE_NAME))?;
    let reader = BufReader::new(file);

    let mut target_line: Option<String> = None;

    if load_line == -1 {
        let mut last_valid_line = None;
        for line_result in reader.lines() {
            let line = line_result?;
            if line.contains(':') {
                last_valid_line = Some(line);
            }
        }
        target_line = last_valid_line;
    } else if load_line > 0 {
        for (i, line_result) in reader.lines().enumerate() {
            if (i + 1) as i64 == load_line {
                target_line = Some(line_result?);
                break;
            }
        }
    }

    match target_line {
        Some(line) => {
            let values: Vec<&str> = line.split(':').collect();
            if values.len() != states.len() {
                return Err(IoError::new(
                    ErrorKind::InvalidData,
                    format!(
                        "State file line has an incorrect number of values: expected {}, got {}",
                        states.len(),
                        values.len()
                    ),
                ));
            }

            for (i, s) in values.iter().enumerate() {
                states[i] = s.trim().parse::<u8>().map_err(|e| {
                    IoError::new(
                        ErrorKind::InvalidData,
                        format!("Failed to parse state value '{}': {}", s, e),
                    )
                })?;
            }
            Ok(())
        }
        None => Err(IoError::new(
            ErrorKind::NotFound,
            format!(
                "State line {} not found in file {}",
                load_line, INIT_TIME_STATES_FILE_NAME
            ),
        )),
    }
}

pub fn write_state(writer: &mut BufWriter<File>, state: &Box<[u8]>) -> IoResult<()> {
    // Get the length of the state array
    let len = state.len();
    // If the array is empty, write only a newline character
    if len == 0 {
        return writer.write_all(b"\n").map(|_| ());
    }

    // Create a buffer with precise capacity: each byte (0 or 1) -> 1 character ('0' or '1') + (len-1) separators ':' + 1 newline character
    let mut buffer = Vec::with_capacity(len + len.saturating_sub(1) + 1);

    // Fill the buffer with values ('0' or '1') and separators ':'
    buffer.extend(state.iter().flat_map(|&val| [val + b'0', b':']));
    // Remove the last superfluous separator ':'
    buffer.pop();
    // Add the newline character
    buffer.push(b'\n');

    // Write the buffer to the file
    writer.write_all(&buffer)?;
    // Return a successful result
    Ok(())
}

pub fn write_f64_state(writer: &mut BufWriter<File>, state: &Vec<f64>) -> IoResult<()> {
    // If the array is empty, write only a newline character
    if state.is_empty() {
        return writer.write_all(b"\n").map(|_| ());
    }

    // Convert all f64 values to strings in scientific notation with 5 decimal places and join with ':' separator
    let line = state
        .iter()
        .map(|v| format!("{:.16e}", v))
        .collect::<Vec<_>>()
        .join(":");

    // Write the line followed by a newline character
    writeln!(writer, "{}", line)?;

    Ok(())
}
