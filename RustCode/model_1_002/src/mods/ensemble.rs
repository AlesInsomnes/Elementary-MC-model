use crate::mods::{
    constants::{K_BOLTZMANN, SIM_LOG_FILE_NAME},
    frontier::Frontier,
    io_handler,
    item::Item,
    lattice::Grid,
    settings::Settings,
    state::SimLog,
    utils,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::{error::Error, io, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Ensemble {
    pub cfg: Settings,
    pub rng: ChaCha8Rng,
    pub grid: Grid,
    pub items: Vec<Item>,
    pub simlog: SimLog,
    pub src_path: PathBuf,
    pub dst_path: PathBuf,
    pub items_len: usize,
    pub items_len0: usize,
}

impl Ensemble {
    pub fn new() -> Result<Self> {
        let exe_dir =
            io_handler::get_exe_dir().map_err(|e| format!("get_exe_dir() failed: {e}"))?;

        let mut cfg = Settings::new();

        io_handler::load_config(&mut cfg, &exe_dir)
            .map_err(|e| format!("Failed to load config from {:?}: {e}", exe_dir))?;
        cfg.validate()?;

        let rng = ChaCha8Rng::seed_from_u64(cfg.seed);

        let grid = Grid::new(cfg.sx, cfg.sy, cfg.sz, cfg.px, cfg.py, cfg.pz);
        let mut simlog = SimLog::new();

        simlog.tot_denergy.is_on = false;
        simlog.cryst_sx.is_on = false;
        simlog.cryst_sy.is_on = false;
        simlog.cryst_sz.is_on = false;

        let mut ensemble = Self {
            cfg,
            rng,
            grid,
            items: Vec::new(),
            simlog: simlog,
            src_path: exe_dir,
            dst_path: PathBuf::new(),
            items_len: 0,
            items_len0: 0,
        };

        ensemble.initialization_stage1()?;
        ensemble.initialization_stage2()?;

        Ok(ensemble)
    }

    fn initialization_stage1(&mut self) -> Result<()> {
        let state_size = self.grid.size;

        let loaded_states_data =
            io_handler::load_states(&self).map_err(|e| format!("Failed to load states: {e}"))?;

        self.items_len0 = loaded_states_data.len();
        self.items_len = self.items_len0;

        self.dst_path = io_handler::prepare_main_dir(&self)
            .map_err(|e| format!("Failed to prepare main dir: {e}"))?;

        let _ = self.simlog.create_out_file(self.dst_path.clone());

        io_handler::prepare_files(&self).map_err(|e| format!("Failed to prepare files: {e}"))?;

        self.items = loaded_states_data
            .into_iter()
            .enumerate()
            .map(|(item_gid, state_data)| {
                let item_dst_path = self.dst_path.join(format!("{:05}", item_gid));
                let mut item = Item::new(item_gid, state_size, item_dst_path)
                    .map_err(|e| format!("Failed to create item {item_gid}: {e}"))?;
                item.state.copy_from_slice(&state_data);
                Ok(item)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    fn initialization_stage2(&mut self) -> Result<()> {
        let cfg = &self.cfg;
        let neibs = &*self.grid.neibs;

        let k_t = K_BOLTZMANN * self.cfg.temperature;
        let n_tot = cfg.n_tot / self.items_len0 as f64;
        let (mode, dg, c_eq, c0, n0_cr, p_b, p_pow) = (
            cfg.mode, cfg.dg, cfg.c_eq, cfg.c0, cfg.n0_cr, cfg.p_b, cfg.p_pow,
        );

        let mut n0_cr_ensemble = 0.0;

        for (item_lid, item) in self.items.iter_mut().enumerate() {
            let n0_cr_calculated = utils::rebuild_front(&*item.state, neibs, &mut item.front);

            let n_cryst0 = { if n0_cr < 0.0 { n0_cr_calculated } else { n0_cr } };

            n0_cr_ensemble += n_cryst0;

            item.simlog
                .initialize(k_t, mode, dg, c_eq, c0, n_tot, n_cryst0, p_b, p_pow);
            // println!("ID: {item_lid} (n_cryst0: {n_cryst0}) --> {:?}", item.state);
        }

        self.simlog.initialize(
            k_t,
            mode,
            dg,
            c_eq,
            c0,
            cfg.n_tot,
            n0_cr_ensemble,
            p_b,
            p_pow,
        );

        for (item_lid, item) in self.items.iter_mut().enumerate() {
            item.simlog.n_gas.is_on = false;
            item.simlog.conc.is_on = false;

            item.simlog.dg.val = self.simlog.dg.val;
            item.write_action(&mut self.grid);
        }

        self.simlog.add_log_point();
        // println!("{:#?}", &self.simlog);

        Ok(())
    }

    pub fn run_simulation(&mut self) -> Result<()> {
        let rng = &mut self.rng;
        let cfg = &self.cfg;
        let grid = &mut self.grid;

        let (ex, ey, ez) = (
            cfg.g100 * cfg.ay * cfg.az,
            cfg.g010 * cfg.ax * cfg.az,
            cfg.g001 * cfg.ax * cfg.ay,
        );
        let (ex2, ey2, ez2) = (ex * 2.0, ey * 2.0, ez * 2.0);
        let eisol = ex2 + ey2 + ez2;

        let (add_check_part, rem_check_part, write_check_part, print_check_part) = (
            cfg.add_i > 0,
            cfg.rem_i > 0,
            cfg.write_i > 0,
            cfg.print_i > 0,
        );

        let mut n_cryst_ensemble = 0.0;
        let mut is_item_alive = true;

        match cfg.mode {
            1.1 | 1.2 | 1.3 => {}
            2.1 | 2.2 | 2.3 => match cfg.mode {
                2.1 => {
                    'simulation_loop: for step_id in 1..=cfg.step_lim {
                        let is_add_step = add_check_part
                            && (step_id >= cfg.add_from)
                            && ((step_id % cfg.add_i) == 0);
                        let is_rem_step = rem_check_part
                            && (step_id >= cfg.rem_from)
                            && ((step_id % cfg.rem_i) == 0);
                        let is_write_step = write_check_part && ((step_id % cfg.write_i) == 0);
                        let is_print_step = print_check_part && ((step_id % cfg.print_i) == 0);

                        n_cryst_ensemble = 0.0;
                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            is_item_alive = item.mode_2_1_step(
                                rng,
                                grid,
                                (ex2, ey2, ez2),
                                step_id,
                                (is_add_step, is_rem_step, is_write_step),
                            );

                            match is_item_alive {
                                true => {
                                    n_cryst_ensemble += item.simlog.n_cryst.val;
                                }
                                false => {
                                    item.write_action(grid);
                                    item.simlog.write_log_to_file()?;
                                    // println!("{:#?}", &self.simlog);
                                }
                            }
                        }

                        self.items.retain(|item| item.is_alive);

                        self.simlog
                            .update_n_sizes(n_cryst_ensemble - self.simlog.n_cryst.val);

                        self.simlog.update_conc_and_dg();

                        if self.items.len() == 0 {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();

                            break 'simulation_loop;
                        }

                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            item.simlog.dg.val = self.simlog.dg.val;
                        }

                        if is_write_step {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();
                        }

                        if is_print_step {
                            println!("Steps: {}/{}", step_id, cfg.step_lim,);
                            // println!(
                            //     "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                            //     sim_state.eq_concentration,
                            //     sim_state.concentration,
                            //     sim_state.n_gas,
                            //     sim_state.n_crystal,
                            //     sim_state.delta_gibbs
                            // );
                        }
                    }
                }
                2.2 => {
                    'simulation_loop: for step_id in 1..=cfg.step_lim {
                        let is_add_step = add_check_part
                            && (step_id >= cfg.add_from)
                            && ((step_id % cfg.add_i) == 0);
                        let is_rem_step = rem_check_part
                            && (step_id >= cfg.rem_from)
                            && ((step_id % cfg.rem_i) == 0);
                        let is_write_step = write_check_part && ((step_id % cfg.write_i) == 0);
                        let is_print_step = print_check_part && ((step_id % cfg.print_i) == 0);

                        n_cryst_ensemble = 0.0;
                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            is_item_alive = item.mode_2_2_step(
                                rng,
                                grid,
                                (ex2, ey2, ez2),
                                step_id,
                                (is_add_step, is_rem_step, is_write_step),
                            );

                            match is_item_alive {
                                true => {
                                    n_cryst_ensemble += item.simlog.n_cryst.val;
                                }
                                false => {
                                    item.write_action(grid);
                                    item.simlog.write_log_to_file()?;
                                    // println!("{:#?}", &self.simlog);
                                }
                            }
                        }

                        self.items.retain(|item| item.is_alive);

                        self.simlog
                            .update_n_sizes(n_cryst_ensemble - self.simlog.n_cryst.val);

                        self.simlog.update_conc_and_dg();

                        if self.items.len() == 0 {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();

                            break 'simulation_loop;
                        }

                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            item.simlog.dg.val = self.simlog.dg.val;
                        }

                        if is_write_step {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();
                        }

                        if is_print_step {
                            println!("Steps: {}/{}", step_id, cfg.step_lim,);
                            // println!(
                            //     "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                            //     sim_state.eq_concentration,
                            //     sim_state.concentration,
                            //     sim_state.n_gas,
                            //     sim_state.n_crystal,
                            //     sim_state.delta_gibbs
                            // );
                        }
                    }
                }
                2.3 => {
                    'simulation_loop: for step_id in 1..=cfg.step_lim {
                        let is_add_step = add_check_part
                            && (step_id >= cfg.add_from)
                            && ((step_id % cfg.add_i) == 0);
                        let is_rem_step = rem_check_part
                            && (step_id >= cfg.rem_from)
                            && ((step_id % cfg.rem_i) == 0);
                        let is_write_step = write_check_part && ((step_id % cfg.write_i) == 0);
                        let is_print_step = print_check_part && ((step_id % cfg.print_i) == 0);

                        n_cryst_ensemble = 0.0;
                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            is_item_alive = item.mode_2_3_step(
                                rng,
                                grid,
                                (ex2, ey2, ez2, eisol),
                                step_id,
                                (is_add_step, is_rem_step, is_write_step),
                            );

                            match is_item_alive {
                                true => {
                                    n_cryst_ensemble += item.simlog.n_cryst.val;
                                }
                                false => {
                                    item.write_action(grid);
                                    item.simlog.write_log_to_file()?;
                                    // println!("{:#?}", &self.simlog);
                                }
                            }
                        }

                        self.items.retain(|item| item.is_alive);

                        self.simlog
                            .update_n_sizes(n_cryst_ensemble - self.simlog.n_cryst.val);

                        self.simlog.update_conc_and_dg();

                        if self.items.len() == 0 {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();

                            break 'simulation_loop;
                        }

                        for (item_lid, item) in self.items.iter_mut().enumerate() {
                            item.simlog.dg.val = self.simlog.dg.val;
                        }

                        if is_write_step {
                            self.simlog.mk_step.val = step_id;
                            self.simlog.add_log_point();
                        }

                        if is_print_step {
                            println!("Steps: {}/{}", step_id, cfg.step_lim,);
                            // println!(
                            //     "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                            //     sim_state.eq_concentration,
                            //     sim_state.concentration,
                            //     sim_state.n_gas,
                            //     sim_state.n_crystal,
                            //     sim_state.delta_gibbs
                            // );
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if self.items.len() > 0 {
            for (item_lid, item) in self.items.iter_mut().enumerate() {
                item.simlog.dg.val = self.simlog.dg.val;
                item.write_action(grid);
                item.simlog.write_log_to_file()?;
            }
        }

        self.simlog.write_log_to_file()?;

        Ok(())
    }
}
