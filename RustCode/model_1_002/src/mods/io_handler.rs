use crate::mods::{
    constants::{
        COMMENT_LINE, CONFIG_FILE_NAME, INIT_TIME_STATES_FILE_NAME, TIME_STATES_FILE_NAME,
    },
    ensemble::Ensemble,
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

pub fn load_config(
    cfg: &mut Settings,
    exe_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(exe_dir.join(CONFIG_FILE_NAME))?;
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

    parse_and_assign_eval!(dispatch, load_option, i64, "LoadOption", number);

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
    let exe_path = current_exe()?;

    exe_path.parent().map(|p| p.to_path_buf()).ok_or_else(|| {
        IoError::new(
            ErrorKind::Other,
            "Failed to get the parent directory of the executable.",
        )
    })
}

fn create_dir_name(ensemble: &Ensemble) -> String {
    let cfg = &ensemble.cfg;
    let timestamp = Utc::now().timestamp_micros();

    let base0 = format!(
        "{}_{}_N{}_X{}Y{}Z{}_T{:e}",
        timestamp, cfg.dir_prefix, ensemble.items_len0, cfg.sx, cfg.sy, cfg.sz, cfg.temperature,
    );

    let base1 = match cfg.mode >= 2.1 {
        false => format!("{}_dg{:e}", base0, cfg.dg),
        true => format!("{}_C{:e}_Nt{:e}", base0, cfg.c0, cfg.n_tot),
    };

    match cfg.mode {
        1.1 | 2.1 => base1,
        1.2 | 2.2 => format!("{}_Pb{:?}", base1, cfg.p_b),
        1.3 | 3.3 => format!("{}_Pb{:?}_Pp{:?}", base1, cfg.p_b, cfg.p_pow),
        _ => base0,
    }
}

pub fn prepare_main_dir(ensemble: &Ensemble) -> IoResult<PathBuf> {
    let dir_name = create_dir_name(&ensemble);
    let res_dir = ensemble.src_path.join(&dir_name);

    fs::create_dir_all(&res_dir).map_err(|e| {
        IoError::new(
            e.kind(),
            format!("Failed to create directory '{}': {}", res_dir.display(), e),
        )
    })?;

    Ok(res_dir)
}

pub fn prepare_files(ensemble: &Ensemble) -> IoResult<()> {
    let path_src_config = ensemble.src_path.join(CONFIG_FILE_NAME);
    let path_dst_config = ensemble.dst_path.join(CONFIG_FILE_NAME);

    if path_src_config.exists() {
        fs::copy(&path_src_config, &path_dst_config).map_err(|e| {
            IoError::new(
                e.kind(),
                format!(
                    "Failed to copy config from '{}' to '{}': {}",
                    path_src_config.display(),
                    path_dst_config.display(),
                    e
                ),
            )
        })?;
    } else {
        eprintln!(
            "⚠️ Warning: Configuration file '{}' not found, skipping copy.",
            path_src_config.display()
        );
    }

    // let path_dst_states = ensemble.dst_path.join(TIME_STATES_FILE_NAME);

    // File::create(&path_dst_states).map_err(|e| {
    //     IoError::new(
    //         ErrorKind::Other,
    //         format!(
    //             "Failed to create file '{}': {}",
    //             path_dst_states.display(),
    //             e
    //         ),
    //     )
    // })?;

    Ok(())
}

pub fn load_states(ensemble: &Ensemble) -> IoResult<Vec<Vec<u8>>> {
    let cfg = &ensemble.cfg;
    let load_line_count = cfg.load_option;
    let load_line_count_usize = load_line_count as usize;

    if load_line_count == 0 {
        return Ok(vec![]);
    }

    let file_path = ensemble.src_path.join(INIT_TIME_STATES_FILE_NAME);
    let reader = BufReader::new(File::open(&file_path)?);

    let expected_len = cfg.sx * cfg.sy * cfg.sz;
    let mut all_lines_data = Vec::new();
    let check1 = load_line_count > 0;

    for (i, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let trimmed = line.trim();

        // Skip empty lines or malformed ones
        if trimmed.is_empty() || !trimmed.contains(':') {
            continue;
        }

        // If we only load a fixed number of lines, stop when reached
        if check1 && all_lines_data.len() >= load_line_count_usize {
            break;
        }

        let values: Vec<u8> = trimmed
            .split(':')
            .filter_map(|s| s.trim().parse::<u8>().ok())
            .collect();

        if values.len() != expected_len {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "Line {} has incorrect number of values: expected {}, got {}",
                    i + 1,
                    expected_len,
                    values.len()
                ),
            ));
        }

        all_lines_data.push(values);
    }

    // Ensure we loaded enough lines if required
    if check1 && all_lines_data.len() < load_line_count_usize {
        return Err(IoError::new(
            ErrorKind::NotFound,
            format!(
                "Expected {} state lines, but found only {} in file {}",
                load_line_count,
                all_lines_data.len(),
                file_path.display()
            ),
        ));
    }

    Ok(all_lines_data)
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

pub fn write_state_uni<T, F>(
    writer: &mut BufWriter<File>,
    state: &[T],
    formatter: &F,
) -> IoResult<()>
where
    T: Copy,
    F: Fn(T) -> String + ?Sized,
{
    if state.is_empty() {
        writer.write_all(b"\n")?;
        return Ok(());
    }

    let line = state
        .iter()
        .copied()
        .map(formatter)
        .collect::<Vec<_>>()
        .join(":");

    writeln!(writer, "{}", line)?;
    Ok(())
}

pub fn write_state_uni_fast<T, F>(
    writer: &mut BufWriter<File>,
    state: &[T],
    formatter: &F,
) -> IoResult<()>
where
    T: Copy,
    F: Fn(T) -> String + ?Sized,
{
    if state.is_empty() {
        writer.write_all(b"\n")?;
        return Ok(());
    }

    let mut first = true;
    for &v in state {
        if !first {
            writer.write_all(b":")?;
        } else {
            first = false;
        }
        writer.write_all(formatter(v).as_bytes())?;
    }
    writer.write_all(b"\n")?;
    Ok(())
}
