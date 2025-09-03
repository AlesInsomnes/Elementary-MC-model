use crate::mods::{
    constants::K_BOLTZMANN,
    frontier::Frontier,
    io_handler,
    lattice::Grid,
    settings::{Settings, SettingsError},
};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::{
    fs::File,
    io::{BufWriter, Error as IoError, ErrorKind, Result as IoResult, Result, Write},
};

struct SimulationState {
    eq_concentration: f64,
    concentration: f64,
    concentration_history: Vec<f64>,
    concentration_negative_counter: u64,
    n_total: f64,
    n_crystal_init: f64,
    n_gas_init: f64,
    n_crystal: f64,
    n_crystal_history: Vec<f64>,
    n_gas: f64,
    n_gas_history: Vec<f64>,
    delta_gibbs: f64,
    delta_gibbs_history: Vec<f64>,
    ballistics_probability: f64,
    total_energy_change: f64,
    energy_change_history: Vec<f64>,
    crystal_sx: f64,
    crystal_sx_history: Vec<f64>,
    crystal_sy: f64,
    crystal_sy_history: Vec<f64>,
    crystal_sz: f64,
    crystal_sz_history: Vec<f64>,
    mk_step: u64,
    mk_step_history: Vec<f64>,
}

impl SimulationState {
    fn new(
        _k_t: f64,
        _ballistics_probability: f64,
        _eq_concentration: f64,
        _concentration: f64,
        _n_total: f64,
        _n_crystal_init: f64,
    ) -> Self {
        let _n_gas_init = _concentration * (_n_total - _n_crystal_init);
        // let _concentration = _n_gas_init / (_n_total - _n_crystal_init);
        let concentration_ratio = _concentration / _eq_concentration;
        let _delta_gibbs = _k_t * concentration_ratio.ln();

        Self {
            eq_concentration: _eq_concentration * 1.0,
            concentration: _concentration * 1.0,
            concentration_history: Vec::new(),
            concentration_negative_counter: 0,
            n_total: _n_total * 1.0,
            n_crystal_init: _n_crystal_init * 1.0,
            n_gas_init: _n_gas_init * 1.0,
            n_crystal: _n_crystal_init * 1.0,
            n_crystal_history: Vec::new(),
            n_gas: _n_gas_init * 1.0,
            n_gas_history: Vec::new(),
            delta_gibbs: _delta_gibbs * 1.0,
            delta_gibbs_history: Vec::new(),
            ballistics_probability: _ballistics_probability * 1.0,
            total_energy_change: 0.0,
            energy_change_history: Vec::new(),
            crystal_sx: -1.0,
            crystal_sx_history: Vec::new(),
            crystal_sy: -1.0,
            crystal_sy_history: Vec::new(),
            crystal_sz: -1.0,
            crystal_sz_history: Vec::new(),
            mk_step: 0,
            mk_step_history: Vec::new(),
        }
    }

    /// Обновляет состояние после события (присоединение/отсоединение).
    /// `particle_change`: 1.0 для присоединения, -1.0 для отсоединения.
    fn update(&mut self, k_t: f64, particle_change: f64) -> bool {
        self.n_crystal += particle_change;
        self.n_gas -= particle_change;

        let concentration = self.n_gas / (self.n_total - self.n_crystal);

        if concentration < 0.0 {
            // Если концентрация стала отрицательной, откатываем изменения и возвращаем ошибку.
            self.concentration_negative_counter += 1;

            self.n_crystal -= particle_change;
            self.n_gas += particle_change;

            return true;
        }

        self.concentration = concentration;

        let concentration_ratio = self.concentration / self.eq_concentration;
        self.delta_gibbs = k_t * concentration_ratio.ln();

        return false;
    }

    fn calculate_energy_change(&mut self, energy_change: f64) {
        self.total_energy_change += energy_change;
    }

    fn measure_crystal_sizes(&mut self, grid: &Grid, front: &Frontier) {
        let mut x_start = usize::MAX;
        let mut x_end = usize::MIN;
        let mut y_start = usize::MAX;
        let mut y_end = usize::MIN;
        let mut z_start = usize::MAX;
        let mut z_end = usize::MIN;

        match front.tpbs_size {
            0 => {
                self.crystal_sx = 0.0;
                self.crystal_sy = 0.0;
                self.crystal_sz = 0.0;
            }
            _ => {
                for &idxg in &front.tpbs[..front.tpbs_size] {
                    let (x, y, z) = grid.idx_to_xyz(idxg);

                    x_start = x_start.min(x);
                    x_end = x_end.max(x);

                    y_start = y_start.min(y);
                    y_end = y_end.max(y);

                    z_start = z_start.min(z);
                    z_end = z_end.max(z);
                }

                self.crystal_sx = (x_end - x_start) as f64 + 1.0;
                self.crystal_sy = (y_end - y_start) as f64 + 1.0;
                self.crystal_sz = (z_end - z_start) as f64 + 1.0;
            }
        }
    }

    fn add_history_point(&mut self) {
        self.n_gas_history.push(self.n_gas);
        self.n_crystal_history.push(self.n_crystal);
        self.concentration_history.push(self.concentration);
        self.delta_gibbs_history.push(self.delta_gibbs);
        self.energy_change_history.push(self.total_energy_change);
        self.crystal_sx_history.push(self.crystal_sx);
        self.crystal_sy_history.push(self.crystal_sy);
        self.crystal_sz_history.push(self.crystal_sz);
        self.mk_step_history.push(self.mk_step as f64);
    }
}

fn sim_mode_1_1(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    sim_state.delta_gibbs = cfg.dg * 1.0;

    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
        }
    }

    Ok(())
}

fn sim_mode_1_2(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    sim_state.delta_gibbs = cfg.dg * 1.0;

    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if sim_state.ballistics_probability > rng.random::<f64>() {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            // d_e = surf_en_change + sim_state.delta_gibbs;

            sim_state.calculate_energy_change(surf_en_change);

            grid.states[idxg] = 0;
            front.tpb_rem(idxg);
            front.tpa_add(idxg);

            let mut has_invalid_neib = false;

            for &neib_idx in idxg_nis.iter() {
                if neib_idx == usize::MAX {
                    has_invalid_neib = true;
                    continue;
                }

                match grid.states[neib_idx] {
                    0 => {
                        if !grid.neibs[neib_idx]
                            .iter()
                            .any(|&n| n != usize::MAX && grid.states[n] == 1)
                        {
                            front.tpa_rem(neib_idx);
                        }
                    }
                    1 => front.tpb_add(neib_idx),
                    _ => {} // Handle unexpected states if necessary
                }
            }

            if has_invalid_neib {
                sim_state.mk_step = step_id;
                println!(
                    "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                    step_id
                );

                break 'simulation_loop;
            }

            let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
            if tpa_len.min(tpb_len) == 0 {
                sim_state.mk_step = step_id;
                eprintln!(
                    "Step: {} -> Ballistic Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                    step_id, tpa_len, tpb_len
                );

                break 'simulation_loop;
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
        }
    }

    Ok(())
}

fn sim_mode_1_3(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    sim_state.delta_gibbs = cfg.dg * 1.0;
    let p_pow = cfg.p_pow;

    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        let tpb_len = front.tpbs_size;
        match tpb_len {
            0 => {
                sim_state.mk_step = step_id;
                eprintln!("TPB Frontier is empty. Simulation stalled or completed.");

                break 'simulation_loop;
            }
            _ => {
                let idxl = rng.random_range(0..tpb_len);
                let idxg = front.tpbs[idxl];
                let idxg_nis = &grid.neibs[idxg];
                let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

                surf_en_change = 0.0;
                match smx_yz {
                    0 => surf_en_change -= ex2,
                    2 => surf_en_change += ex2,
                    _ => {}
                }
                match smy_xz {
                    0 => surf_en_change -= ey2,
                    2 => surf_en_change += ey2,
                    _ => {}
                }
                match smz_xy {
                    0 => surf_en_change -= ez2,
                    2 => surf_en_change += ez2,
                    _ => {}
                }
                // d_e = surf_en_change + sim_state.delta_gibbs;

                let prob = sim_state.ballistics_probability
                    * (1.0f64 - (surf_en_change / eisol)).powf(p_pow);
                // // println!("E: {:.5e}; P: {:.5e}", energy, prob);
                if prob > rng.random::<f64>() {
                    // sim_state.calculate_energy_change(d_e + eisol);
                    sim_state.calculate_energy_change(surf_en_change);

                    grid.states[idxg] = 0;
                    front.tpb_rem(idxg);
                    front.tpa_add(idxg);

                    let mut has_invalid_neib = false;

                    for &neib_idx in idxg_nis.iter() {
                        if neib_idx == usize::MAX {
                            has_invalid_neib = true;
                            continue;
                        }

                        match grid.states[neib_idx] {
                            0 => {
                                if !grid.neibs[neib_idx]
                                    .iter()
                                    .any(|&n| n != usize::MAX && grid.states[n] == 1)
                                {
                                    front.tpa_rem(neib_idx);
                                }
                            }
                            1 => front.tpb_add(neib_idx),
                            _ => {} // Handle unexpected states if necessary
                        }
                    }

                    if has_invalid_neib {
                        sim_state.mk_step = step_id;
                        println!(
                            "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                            step_id
                        );

                        break 'simulation_loop;
                    }

                    let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                    if tpa_len.min(tpb_len) == 0 {
                        sim_state.mk_step = step_id;
                        eprintln!(
                            "Step: {} -> Ballistic Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                            step_id, tpa_len, tpb_len
                        );

                        break 'simulation_loop;
                    }
                }
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
        }
    }

    Ok(())
}

fn sim_mode_2_1(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    println!(
        "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
        sim_state.eq_concentration,
        sim_state.concentration,
        sim_state.n_gas,
        sim_state.n_crystal,
        sim_state.delta_gibbs
    );

    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, 1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e - eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, -1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e + eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
            println!(
                "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                sim_state.eq_concentration,
                sim_state.concentration,
                sim_state.n_gas,
                sim_state.n_crystal,
                sim_state.delta_gibbs
            );
        }
    }

    Ok(())
}

fn sim_mode_2_2(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    println!(
        "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
        sim_state.eq_concentration,
        sim_state.concentration,
        sim_state.n_gas,
        sim_state.n_crystal,
        sim_state.delta_gibbs
    );

    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, 1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e - eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, -1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e + eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if sim_state.ballistics_probability > rng.random::<f64>() {
            let tpb_len = front.tpbs_size;
            if tpb_len == 0 {
                sim_state.mk_step = step_id;
                eprintln!("TPB Frontier is empty. Simulation stalled or completed.");

                break 'simulation_loop;
            }

            let not_accepted = sim_state.update(k_t, -1.0);

            if not_accepted {
                continue 'simulation_loop;
            }

            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            // d_e = surf_en_change + sim_state.delta_gibbs;

            sim_state.calculate_energy_change(surf_en_change);

            grid.states[idxg] = 0;
            front.tpb_rem(idxg);
            front.tpa_add(idxg);

            let mut has_invalid_neib = false;

            for &neib_idx in idxg_nis.iter() {
                if neib_idx == usize::MAX {
                    has_invalid_neib = true;
                    continue;
                }

                match grid.states[neib_idx] {
                    0 => {
                        if !grid.neibs[neib_idx]
                            .iter()
                            .any(|&n| n != usize::MAX && grid.states[n] == 1)
                        {
                            front.tpa_rem(neib_idx);
                        }
                    }
                    1 => front.tpb_add(neib_idx),
                    _ => {} // Handle unexpected states if necessary
                }
            }

            if has_invalid_neib {
                sim_state.mk_step = step_id;
                println!(
                    "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                    step_id
                );

                break 'simulation_loop;
            }

            let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
            if tpa_len.min(tpb_len) == 0 {
                sim_state.mk_step = step_id;
                eprintln!(
                    "Step: {} -> Ballistic Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                    step_id, tpa_len, tpb_len
                );

                break 'simulation_loop;
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
            println!(
                "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                sim_state.eq_concentration,
                sim_state.concentration,
                sim_state.n_gas,
                sim_state.n_crystal,
                sim_state.delta_gibbs
            );
        }
    }

    Ok(())
}

fn sim_mode_2_3(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
    sim_state: &mut SimulationState,
    print_check_part: bool,
    write_check_part: bool,
    add_check_part: bool,
    add_i: u64,
    add_from: u64,
    rem_check_part: bool,
    rem_i: u64,
    rem_from: u64,
    k_t: f64,
    ex2: f64,
    ey2: f64,
    ez2: f64,
    eisol: f64,
) -> Result<()> {
    println!(
        "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
        sim_state.eq_concentration,
        sim_state.concentration,
        sim_state.n_gas,
        sim_state.n_crystal,
        sim_state.delta_gibbs
    );

    let p_pow = cfg.p_pow;
    let (mut surf_en_change, mut d_e);
    'simulation_loop: for step_id in 1..=cfg.step_lim {
        let is_add_step = add_check_part && (step_id >= add_from) && (step_id % add_i == 0);
        let is_rem_step = rem_check_part && (step_id >= rem_from) && (step_id % rem_i == 0);

        if is_add_step {
            let tpa_len = front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = front.tpas[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change += ex2,
                2 => surf_en_change -= ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change += ey2,
                2 => surf_en_change -= ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change += ez2,
                2 => surf_en_change -= ez2,
                _ => {}
            }
            d_e = surf_en_change - sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, 1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e - eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 1;
                front.tpa_rem(idxg);
                front.tpb_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => front.tpa_add(neib_idx),
                        1 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 0)
                            {
                                front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Add action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        if is_rem_step {
            let tpb_len = front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = front.tpbs[idxl];
            let idxg_nis = &grid.neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

            surf_en_change = 0.0;
            match smx_yz {
                0 => surf_en_change -= ex2,
                2 => surf_en_change += ex2,
                _ => {}
            }
            match smy_xz {
                0 => surf_en_change -= ey2,
                2 => surf_en_change += ey2,
                _ => {}
            }
            match smz_xy {
                0 => surf_en_change -= ez2,
                2 => surf_en_change += ez2,
                _ => {}
            }
            d_e = surf_en_change + sim_state.delta_gibbs;

            if d_e < 0.0 || (-d_e / k_t).exp() > rng.random::<f64>() {
                let not_accepted = sim_state.update(k_t, -1.0);

                if not_accepted {
                    continue 'simulation_loop;
                }

                // sim_state.calculate_energy_change(d_e + eisol);
                sim_state.calculate_energy_change(surf_en_change);

                grid.states[idxg] = 0;
                front.tpb_rem(idxg);
                front.tpa_add(idxg);

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match grid.states[neib_idx] {
                        0 => {
                            if !grid.neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && grid.states[n] == 1)
                            {
                                front.tpa_rem(neib_idx);
                            }
                        }
                        1 => front.tpb_add(neib_idx),
                        _ => {} // Handle unexpected states if necessary
                    }
                }

                if has_invalid_neib {
                    sim_state.mk_step = step_id;
                    println!(
                        "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                        step_id
                    );

                    break 'simulation_loop;
                }

                let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                if tpa_len.min(tpb_len) == 0 {
                    sim_state.mk_step = step_id;
                    eprintln!(
                        "Step: {} -> Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                        step_id, tpa_len, tpb_len
                    );

                    break 'simulation_loop;
                }
            }
        }

        let tpb_len = front.tpbs_size;
        match tpb_len {
            0 => {
                sim_state.mk_step = step_id;
                eprintln!("TPB Frontier is empty. Simulation stalled or completed.");

                break 'simulation_loop;
            }
            _ => {
                let idxl = rng.random_range(0..tpb_len);
                let idxg = front.tpbs[idxl];
                let idxg_nis = &grid.neibs[idxg];
                let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&grid.states, idxg_nis);

                surf_en_change = 0.0;
                match smx_yz {
                    0 => surf_en_change -= ex2,
                    2 => surf_en_change += ex2,
                    _ => {}
                }
                match smy_xz {
                    0 => surf_en_change -= ey2,
                    2 => surf_en_change += ey2,
                    _ => {}
                }
                match smz_xy {
                    0 => surf_en_change -= ez2,
                    2 => surf_en_change += ez2,
                    _ => {}
                }
                // d_e = surf_en_change + sim_state.delta_gibbs;

                let prob = sim_state.ballistics_probability
                    * (1.0f64 - (surf_en_change / eisol)).powf(p_pow);
                // // println!("E: {:.5e}; P: {:.5e}", energy, prob);
                if prob > rng.random::<f64>() {
                    let not_accepted = sim_state.update(k_t, -1.0);

                    if not_accepted {
                        continue 'simulation_loop;
                    }

                    // sim_state.calculate_energy_change(d_e + eisol);
                    sim_state.calculate_energy_change(surf_en_change);

                    grid.states[idxg] = 0;
                    front.tpb_rem(idxg);
                    front.tpa_add(idxg);

                    let mut has_invalid_neib = false;

                    for &neib_idx in idxg_nis.iter() {
                        if neib_idx == usize::MAX {
                            has_invalid_neib = true;
                            continue;
                        }

                        match grid.states[neib_idx] {
                            0 => {
                                if !grid.neibs[neib_idx]
                                    .iter()
                                    .any(|&n| n != usize::MAX && grid.states[n] == 1)
                                {
                                    front.tpa_rem(neib_idx);
                                }
                            }
                            1 => front.tpb_add(neib_idx),
                            _ => {} // Handle unexpected states if necessary
                        }
                    }

                    if has_invalid_neib {
                        sim_state.mk_step = step_id;
                        println!(
                            "Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
                            step_id
                        );

                        break 'simulation_loop;
                    }

                    let (tpa_len, tpb_len) = (front.tpas_size, front.tpbs_size);
                    if tpa_len.min(tpb_len) == 0 {
                        sim_state.mk_step = step_id;
                        eprintln!(
                            "Step: {} -> Ballistic Rem action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
                            step_id, tpa_len, tpb_len
                        );

                        break 'simulation_loop;
                    }
                }
            }
        }

        sim_state.mk_step = step_id;

        if should_perform_action(step_id, cfg.write_i, write_check_part) {
            io_handler::write_state(dst_states_buf, &grid.states)?;
            dst_states_buf.flush()?;

            sim_state.measure_crystal_sizes(&grid, &front);
            sim_state.add_history_point();
        }

        if should_perform_action(step_id, cfg.print_i, print_check_part) {
            println!(
                "Steps: {}/{} | TPA: {} TPB: {}",
                step_id, cfg.step_lim, front.tpas_size, front.tpbs_size,
            );
            println!(
                "Ceq: {:.5e}; C: {:.5e}; nv_gas: {:.5e}; nv_cryst: {:.5e}; dg: {:.5e}",
                sim_state.eq_concentration,
                sim_state.concentration,
                sim_state.n_gas,
                sim_state.n_crystal,
                sim_state.delta_gibbs
            );
        }
    }

    Ok(())
}

pub fn run_calculations(
    cfg: &Settings,
    grid: &mut Grid,
    front: &mut Frontier,
    rng: &mut ChaCha8Rng,
    dst_states_buf: &mut BufWriter<File>,
) -> Result<()> {
    let path_out_file_1 = cfg.dst_path.join("sim_history.txt");
    let out_file_1 = File::create(path_out_file_1)?;
    let mut out_file_1_buf = BufWriter::new(out_file_1);

    let k_t = K_BOLTZMANN * cfg.temperature;
    let (ex, ey, ez) = (
        cfg.g100 * cfg.ay * cfg.az,
        cfg.g010 * cfg.ax * cfg.az,
        cfg.g001 * cfg.ax * cfg.ay,
    );
    let (ex2, ey2, ez2) = (ex * 2.0, ey * 2.0, ez * 2.0);
    let eisol = ex2 + ey2 + ez2;

    activate_center(cfg, grid)?;
    let n_cr_calculated = rebuild_front(grid, front);

    io_handler::write_state(dst_states_buf, &grid.states)?;
    dst_states_buf.flush()?;

    let (add_check_part, rem_check_part, write_check_part, print_check_part) = (
        cfg.add_i > 0,
        cfg.rem_i > 0,
        cfg.write_i > 0,
        cfg.print_i > 0,
    );

    let n_cr = {
        if cfg.n0_cr <= 0.0 {
            n_cr_calculated
        } else {
            cfg.n0_cr
        }
    };

    let mut sim_state = SimulationState::new(k_t, cfg.p_b, cfg.c_eq, cfg.c0, cfg.n_tot, n_cr);
    sim_state.measure_crystal_sizes(&grid, &front);
    sim_state.add_history_point();

    match cfg.mode {
        1.1 => {
            let _ = sim_mode_1_1(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        1.2 => {
            let _ = sim_mode_1_2(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        1.3 => {
            let _ = sim_mode_1_3(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        2.1 => {
            let _ = sim_mode_2_1(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        2.2 => {
            let _ = sim_mode_2_2(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        2.3 => {
            let _ = sim_mode_2_3(
                &cfg,
                grid,
                front,
                rng,
                dst_states_buf,
                &mut sim_state,
                print_check_part,
                write_check_part,
                add_check_part,
                cfg.add_i,
                cfg.add_from,
                rem_check_part,
                cfg.rem_i,
                cfg.rem_from,
                k_t,
                ex2,
                ey2,
                ez2,
                eisol,
            );
        }
        _ => {}
    }

    io_handler::write_state(dst_states_buf, &grid.states)?;
    dst_states_buf.flush()?;

    sim_state.measure_crystal_sizes(&grid, &front);
    sim_state.add_history_point();

    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.n_gas_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.n_crystal_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.concentration_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.delta_gibbs_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.energy_change_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.crystal_sx_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.crystal_sy_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.crystal_sz_history)?;
    io_handler::write_f64_state(&mut out_file_1_buf, &sim_state.mk_step_history)?;
    out_file_1_buf.flush()?;

    Ok(())
}

#[inline(always)]
fn should_perform_action(step_id: u64, interval: u64, pre_flag: bool) -> bool {
    pre_flag && ((step_id % interval) == 0)
}

#[inline(always)]
fn activate_center(cfg: &Settings, grid: &mut Grid) -> IoResult<()> {
    let center_id = grid.xyz_to_idx(cfg.sx / 2, cfg.sy / 2, cfg.sz / 2);

    if center_id >= grid.size {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "Center index out of bounds",
        ));
    }

    grid.states[center_id] = 1;

    Ok(())
}

#[inline(always)]
fn rebuild_front(grid: &Grid, front: &mut Frontier) -> f64 {
    println!("Обновление фронтов газа и кластера...");
    let states = &grid.states;
    let neibs = &grid.neibs;

    let mut n_cr_calculated = 0.0;

    for (i, &state) in states.iter().enumerate() {
        if state == 1 {
            n_cr_calculated += 1.0;

            let mut has_vacancy_neighbor = false;

            for neib_idx in neibs[i] {
                if neib_idx != usize::MAX {
                    if states[neib_idx] == 0 {
                        has_vacancy_neighbor = true;
                        front.tpa_add(neib_idx);
                    }
                }
            }

            if has_vacancy_neighbor {
                front.tpb_add(i);
            }
        }
    }
    println!(
        "Обновление завершено! Узлов фронта газа: {}, Узлов фронта кластера: {}",
        front.tpas_size, front.tpbs_size
    );

    return n_cr_calculated;
}

#[inline(always)]
fn compute_neighbor_sums(states: &[u8], idxg_nis: &[usize; 6]) -> (u8, u8, u8) {
    let mut x_axis_neighbors = 0;
    let mut y_axis_neighbors = 0;
    let mut z_axis_neighbors = 0;

    for i in 0..6 {
        let idx = unsafe { *idxg_nis.get_unchecked(i) };
        if idx != usize::MAX {
            let state = unsafe { *states.get_unchecked(idx) };
            if state == 1 {
                match i {
                    0 | 1 => x_axis_neighbors += 1,
                    2 | 3 => y_axis_neighbors += 1,
                    4 | 5 => z_axis_neighbors += 1,
                    _ => unreachable!(), // Цикл всегда от 0 до 5
                }
            }
        }
    }

    (x_axis_neighbors, y_axis_neighbors, z_axis_neighbors)
}
