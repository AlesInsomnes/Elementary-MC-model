use crate::mods::{constants::SIM_LOG_FILE_NAME, frontier::Frontier, io_handler, lattice::Grid};

use std::{
    fmt::Debug,
    fs::File,
    io::{BufWriter, Error as IoError, Result as IoResult, Write},
    path::PathBuf,
};

pub struct LogEntry<T: Debug + 'static> {
    pub val: T,
    pub log: Vec<T>,
    pub is_on: bool,
    pub format_f: Box<dyn Fn(T) -> String + 'static>,
}

impl<T: Debug + 'static> Debug for LogEntry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogEntry")
            .field("val", &self.val)
            .field("log", &self.log)
            .field("is_on", &self.is_on)
            .field("format_f", &"<closure>")
            .finish()
    }
}

impl<T: Debug + Copy + 'static> LogEntry<T> {
    pub fn new<F>(val: T, is_on: bool, format_f: F) -> Self
    where
        F: Fn(T) -> String + 'static,
    {
        Self {
            val,
            log: Vec::new(),
            is_on,
            format_f: Box::new(format_f),
        }
    }

    pub fn push_if_enabled(&mut self) {
        if self.is_on {
            self.log.push(self.val);
        }
    }
}
#[derive(Debug)]
pub struct SimLog {
    pub k_t: f64,
    pub p_b: f64,
    pub p_pow: f64,

    pub conc_eq: f64,
    pub conc: LogEntry<f64>,
    pub conc_neg_count: u64,

    pub n_tot: f64,

    pub n_cryst: LogEntry<f64>,
    pub n_gas: LogEntry<f64>,
    pub dg: LogEntry<f64>,
    pub tot_denergy: LogEntry<f64>,

    pub cryst_sx: LogEntry<usize>,
    pub cryst_sy: LogEntry<usize>,
    pub cryst_sz: LogEntry<usize>,
    pub mk_step: LogEntry<u64>,

    pub path_out_file: Option<PathBuf>,
    pub out_file_buf: Option<BufWriter<File>>,
}

impl SimLog {
    pub fn new() -> Self {
        let fmt1 = |v: f64| format!("{:.15e}", v);
        let fmt2 = |v: usize| v.to_string();
        let fmt3 = |v: u64| v.to_string();

        Self {
            k_t: 0.0,
            p_b: 0.0,
            p_pow: 0.0,

            conc_eq: 0.0,
            conc: LogEntry::new(0.0, false, fmt1),
            conc_neg_count: 0,

            n_tot: 0.0,

            n_cryst: LogEntry::new(0.0, true, fmt1),
            n_gas: LogEntry::new(0.0, false, fmt1),
            dg: LogEntry::new(0.0, false, fmt1),
            tot_denergy: LogEntry::new(0.0, true, fmt1),
            cryst_sx: LogEntry::new(0, true, fmt2),
            cryst_sy: LogEntry::new(0, true, fmt2),
            cryst_sz: LogEntry::new(0, true, fmt2),
            mk_step: LogEntry::new(0, true, fmt3),

            path_out_file: None,
            out_file_buf: None,
        }
    }

    pub fn create_out_file(&mut self, path_dst: PathBuf) -> IoResult<()> {
        let path_out_file = path_dst.join(SIM_LOG_FILE_NAME);

        let out_file_buf = BufWriter::new(File::create(&path_out_file).map_err(|e| {
            IoError::new(
                e.kind(),
                format!("Failed to create file '{}': {}", path_out_file.display(), e),
            )
        })?);

        self.path_out_file = Some(path_out_file);
        self.out_file_buf = Some(out_file_buf);

        Ok(())
    }

    pub fn initialize(
        &mut self,
        k_t: f64,
        sim_mode: f64,
        dg0: f64,
        conc_eq: f64,
        conc0: f64,
        n_tot: f64,
        n_cryst0: f64,
        p_b: f64,
        p_pow: f64,
    ) {
        self.k_t = k_t;
        self.p_b = p_b;
        self.p_pow = p_pow;

        self.conc_eq = conc_eq;
        self.conc.val = conc0;

        self.n_tot = n_tot;

        self.n_cryst.val = n_cryst0;
        self.dg.val = dg0;

        if sim_mode >= 2.1 {
            let n_gas0 = conc0 * (n_tot - n_cryst0);
            let conc_ratio = conc0 / conc_eq;
            let dg0 = k_t * conc_ratio.ln();

            self.n_gas.val = n_gas0;
            self.dg.val = dg0;

            self.conc.is_on = true;
            self.n_gas.is_on = true;
            self.dg.is_on = true;
        }
    }

    // pub fn update(&mut self, k_t: f64, particle_change: f64) -> bool {
    //     self.n_cryst += particle_change;
    //     self.n_gas -= particle_change;

    //     let conc = self.n_gas / (self.n_tot - self.n_cryst);

    //     if conc < 0.0 {
    //         self.conc_neg_count += 1;

    //         self.n_cryst -= particle_change;
    //         self.n_gas += particle_change;

    //         return true;
    //     }

    //     self.conc = conc;

    //     let conc_ratio = self.conc / self.conc_eq;
    //     self.dg = k_t * conc_ratio.ln();

    //     return false;
    // }

    pub fn update_n_sizes(&mut self, dn_cryst: f64) {
        self.n_cryst.val += dn_cryst;
        self.n_gas.val -= dn_cryst;
    }

    pub fn update_conc(&mut self) {
        self.conc.val = self.n_gas.val / (self.n_tot - self.n_cryst.val);

        if self.conc.val < 0.0 {
            self.conc_neg_count += 1;
        }
    }

    pub fn update_dg(&mut self) {
        let conc_ratio = self.conc.val / self.conc_eq;
        self.dg.val = self.k_t * conc_ratio.ln();
    }

    pub fn update_conc_and_dg(&mut self) {
        self.update_conc();
        self.update_dg();
    }

    pub fn add_denergy(&mut self, tot_denergy: f64) {
        self.tot_denergy.val += tot_denergy;
    }

    pub fn measure_cryst_sizes(&mut self, grid: &mut Grid, front: &Frontier) {
        if front.tpbs_size == 0 {
            self.cryst_sx.val = 0;
            self.cryst_sy.val = 0;
            self.cryst_sz.val = 0;
            return;
        }

        grid.nx_ib.fill(0);
        grid.ny_ib.fill(0);
        grid.nz_ib.fill(0);

        for &idxg in front.tpbs.iter().take(front.tpbs_size) {
            let (x, y, z) = grid.idx_to_xyz(idxg);

            grid.nx_ib[x] = 1;
            grid.ny_ib[y] = 1;
            grid.nz_ib[z] = 1;
        }

        self.cryst_sx.val = grid.nx_ib.iter().sum();
        self.cryst_sy.val = grid.ny_ib.iter().sum();
        self.cryst_sz.val = grid.nz_ib.iter().sum();
    }

    pub fn add_log_point(&mut self) {
        self.n_gas.push_if_enabled();
        self.n_cryst.push_if_enabled();
        self.conc.push_if_enabled();
        self.dg.push_if_enabled();
        self.tot_denergy.push_if_enabled();
        self.cryst_sx.push_if_enabled();
        self.cryst_sy.push_if_enabled();
        self.cryst_sz.push_if_enabled();
        self.mk_step.push_if_enabled();
    }

    pub fn write_log_to_file(&mut self) -> IoResult<()> {
        if let Some(buf) = &mut self.out_file_buf {
            io_handler::write_state_uni(buf, &self.n_gas.log, &self.n_gas.format_f)?;
            io_handler::write_state_uni(buf, &self.n_cryst.log, &self.n_cryst.format_f)?;
            io_handler::write_state_uni(buf, &self.conc.log, &self.conc.format_f)?;
            io_handler::write_state_uni(buf, &self.dg.log, &self.dg.format_f)?;
            io_handler::write_state_uni(buf, &self.tot_denergy.log, &self.tot_denergy.format_f)?;
            io_handler::write_state_uni(buf, &self.cryst_sx.log, &self.cryst_sx.format_f)?;
            io_handler::write_state_uni(buf, &self.cryst_sy.log, &self.cryst_sy.format_f)?;
            io_handler::write_state_uni(buf, &self.cryst_sz.log, &self.cryst_sz.format_f)?;
            io_handler::write_state_uni(buf, &self.mk_step.log, &self.mk_step.format_f)?;

            buf.flush()?;
            Ok(())
        } else {
            eprintln!("Error: Log file not initialized!");
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Log file not initialized",
            ))
        }
    }
}
