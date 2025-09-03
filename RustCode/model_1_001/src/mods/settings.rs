use crate::mods::io_handler::get_exe_dir;
use std::{borrow::Cow, error::Error, fmt, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub dir_prefix: String,
    pub seed: u64,

    pub sx: usize,
    pub sy: usize,
    pub sz: usize,
    pub px: bool,
    pub py: bool,
    pub pz: bool,

    pub temperature: f64,
    pub ax: f64,
    pub ay: f64,
    pub az: f64,

    pub g100: f64,
    pub g010: f64,
    pub g001: f64,

    pub mode: f64,
    pub dg: f64,
    pub c_eq: f64,
    pub c0: f64,
    pub n_tot: f64,
    pub n0_cr: f64,
    pub p_b: f64,
    pub p_pow: f64,

    pub add_i: u64,
    pub add_from: u64,
    pub rem_i: u64,
    pub rem_from: u64,

    pub load_prev: i64, // 0 means generate new, >0 means load specific line, -1 means load last line

    pub step_lim: u64,
    pub print_i: u64,
    pub write_i: u64,

    pub src_path: PathBuf,
    pub dst_path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        let exe_dir = get_exe_dir().expect("REASON");

        Self {
            dir_prefix: "Default".to_string(),
            seed: 1012,

            sx: 11,
            sy: 11,
            sz: 11,
            px: false,
            py: false,
            pz: false,

            temperature: 300.0,
            ax: 5.85E-10,
            ay: 1.78E-10,
            az: 4.41E-10,

            g100: 0.41,
            g010: 0.54,
            g001: 0.22,

            mode: 1.1,
            dg: 0.0,
            c_eq: 9.58767e-08,
            c0: 9.58767e-08,
            n_tot: 5e12,
            n0_cr: -1.0,
            p_b: 0.3,
            p_pow: 1.0,

            add_i: 1,
            add_from: 1,
            rem_i: 1,
            rem_from: 1,

            load_prev: 0, // 0 means generate new, >0 means load specific line, -1 means load last line

            step_lim: 100,
            print_i: 10,
            write_i: 1,

            src_path: exe_dir,
            dst_path: PathBuf::new(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.sx == 0 {
            return Err(SettingsError::simple("Sx", "must be > 0"));
        }
        if self.sy == 0 {
            return Err(SettingsError::simple("Sy", "must be > 0"));
        }
        if self.sz == 0 {
            return Err(SettingsError::simple("Sz", "must be > 0"));
        }
        if self.temperature <= 0.0 {
            return Err(SettingsError::simple("T", "must be > 0"));
        }
        if self.add_from < 1 {
            return Err(SettingsError::simple("AddFrom", "must be > 0"));
        }
        if self.rem_from < 1 {
            return Err(SettingsError::simple("RemFrom", "must be > 0"));
        }
        // if self.ax <= 0.0 || self.ay <= 0.0 || self.az <= 0.0 {
        //     return Err(SettingsError::simple("Ax/Ay/Az", "must be > 0"));
        // }
        // if self.step_lim == 0 {
        //     return Err(SettingsError::simple("GenStepsLimit", "must be > 0"));
        // }
        // if self.write_on_step == 0 {
        //     return Err(SettingsError::simple("WriteOnStep", "must be > 0"));
        // }
        // if self.print_on_step == 0 {
        //     return Err(SettingsError::simple("PrintOnStep", "must be > 0"));
        // }
        if self.dir_prefix.trim().is_empty() {
            return Err(SettingsError::simple("DirPrefix", "cannot be empty"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SettingsError {
    pub key: Option<Cow<'static, str>>,
    pub value: Option<String>,
    pub source: Box<dyn Error + Send + Sync>,
}

impl SettingsError {
    pub fn new<K: Into<Cow<'static, str>>, V: Into<String>, E: Error + Send + Sync + 'static>(
        key: K,
        value: V,
        source: E,
    ) -> Self {
        Self {
            key: Some(key.into()),
            value: Some(value.into()),
            source: Box::new(source),
        }
    }

    pub fn simple<K: Into<Cow<'static, str>>, M: Into<String>>(key: K, message: M) -> Self {
        Self {
            key: Some(key.into()),
            value: None,
            source: Box::new(SimpleMsg(message.into())),
        }
    }

    pub fn from_io(err: std::io::Error) -> Self {
        Self {
            key: None,
            value: None,
            source: Box::new(err),
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.key, &self.value) {
            (Some(key), Some(value)) => {
                write!(
                    f,
                    "Failed to parse '{}' with value '{}': {}",
                    key, value, self.source
                )
            }
            (Some(key), None) => {
                write!(f, "Invalid value for '{}': {}", key, self.source)
            }
            _ => write!(f, "Settings error: {}", self.source),
        }
    }
}

impl Error for SettingsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

impl From<std::io::Error> for SettingsError {
    fn from(err: std::io::Error) -> Self {
        Self::from_io(err)
    }
}

#[derive(Debug)]
struct SimpleMsg(String);

impl fmt::Display for SimpleMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SimpleMsg {}
