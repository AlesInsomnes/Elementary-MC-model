use crate::mods::{
    constants::{SIM_LOG_FILE_NAME, TIME_STATES_FILE_NAME},
    frontier::Frontier,
    io_handler,
    lattice::Grid,
    settings::Settings,
    state::SimLog,
    utils::compute_neighbor_sums,
};
use rand::SeedableRng;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Error as IoError, ErrorKind, Result as IoResult, Write},
    path::PathBuf,
};

#[derive(Debug)]
pub struct Item {
    pub item_gid: usize,
    pub is_alive: bool,
    pub state: Box<[u8]>,
    pub front: Frontier,
    pub simlog: SimLog,
    pub path_dst: PathBuf,
    pub path_time_states: PathBuf,
    pub time_states_fbuf: BufWriter<File>,
}

impl Item {
    pub fn new(item_gid: usize, size: usize, dst_dir: PathBuf) -> IoResult<Self> {
        fs::create_dir_all(&dst_dir).map_err(|e| {
            IoError::new(
                e.kind(),
                format!("Failed to create directory '{}': {}", dst_dir.display(), e),
            )
        })?;

        let path_time_states = dst_dir.join(TIME_STATES_FILE_NAME);
        let time_states_fbuf = BufWriter::new(File::create(&path_time_states).map_err(|e| {
            IoError::new(
                e.kind(),
                format!(
                    "Failed to create file '{}': {}",
                    path_time_states.display(),
                    e
                ),
            )
        })?);

        let mut simlog = SimLog::new();
        let _ = simlog.create_out_file(dst_dir.clone());

        let state = vec![0; size].into_boxed_slice();
        let front = Frontier::new(size);

        Ok(Self {
            item_gid,
            is_alive: true,
            state,
            front,
            simlog,
            path_dst: dst_dir,
            path_time_states,
            time_states_fbuf,
        })
    }

    fn is_front_empty(&self) -> bool {
        self.front.tpas_size == 0 || self.front.tpbs_size == 0
    }

    fn handle_stalled_front(&mut self, step_id: u64, action: &str) {
        self.simlog.mk_step.val = step_id;
        eprintln!(
            "[Item ID: {:05}] Step: {} -> {} action. Found an empty Front: | TPA: {} - TPB: {} |.\nSimulation stalled or completed.",
            self.item_gid, step_id, action, self.front.tpas_size, self.front.tpbs_size
        );
        self.is_alive = false;
    }

    fn handle_stalled_boundary(&mut self, step_id: u64) {
        self.simlog.mk_step.val = step_id;
        println!(
            "[Item ID: {:05}] Step: {} -> Status: Sample boundary cell found in neighbors.\nSimulation stalled or completed.",
            self.item_gid, step_id
        );
        self.is_alive = false;
    }

    pub fn write_action(&mut self, grid: &mut Grid) {
        let _ = io_handler::write_state(&mut self.time_states_fbuf, &self.state);
        let _ = self.time_states_fbuf.flush();

        self.simlog.measure_cryst_sizes(grid, &self.front);
        self.simlog.add_log_point();
    }

    // pub fn mode_1_1_step(
    //     &mut self,
    //     step_id: u64,
    //     (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    //     rng: &mut ChaCha8Rng,
    //     neibs: &[[usize; 6]],
    //     grid: &mut Grid,
    //     (ex2, ey2, ez2): (f64, f64, f64),
    // ) -> bool {
    //     let (mut surf_en_change, mut d_e) = (0.0, 0.0);

    //     if is_add_step {
    //         let tpa_len = self.front.tpas_size;
    //         let idxl = rng.random_range(0..tpa_len);
    //         let idxg = self.front.tpas[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change += ex2,
    //             2 => surf_en_change -= ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change += ey2,
    //             2 => surf_en_change -= ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change += ez2,
    //             2 => surf_en_change -= ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change - self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 1;
    //             self.front.tpa_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) < 6 {
    //                 self.front.tpb_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => self.front.tpa_add(neib_idx),
    //                     1 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 0)
    //                         {
    //                             self.front.tpb_rem(neib_idx);
    //                         }
    //                     }
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Add");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     if is_rem_step {
    //         let tpb_len = self.front.tpbs_size;
    //         let idxl = rng.random_range(0..tpb_len);
    //         let idxg = self.front.tpbs[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change -= ex2,
    //             2 => surf_en_change += ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change -= ey2,
    //             2 => surf_en_change += ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change -= ez2,
    //             2 => surf_en_change += ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change + self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 0;
    //             self.front.tpb_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) > 0 {
    //                 self.front.tpa_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 1)
    //                         {
    //                             self.front.tpa_rem(neib_idx);
    //                         }
    //                     }
    //                     1 => self.front.tpb_add(neib_idx),
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Rem");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     self.simlog.mk_step.val = step_id;

    //     if is_write_step {
    //         self.write_action(grid);
    //     }

    //     self.is_alive
    // }

    // pub fn mode_1_2_step(
    //     &mut self,
    //     step_id: u64,
    //     (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    //     rng: &mut ChaCha8Rng,
    //     neibs: &[[usize; 6]],
    //     grid: &mut Grid,
    //     (ex2, ey2, ez2): (f64, f64, f64),
    // ) -> bool {
    //     let (mut surf_en_change, mut d_e) = (0.0, 0.0);

    //     if is_add_step {
    //         let tpa_len = self.front.tpas_size;
    //         let idxl = rng.random_range(0..tpa_len);
    //         let idxg = self.front.tpas[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change += ex2,
    //             2 => surf_en_change -= ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change += ey2,
    //             2 => surf_en_change -= ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change += ez2,
    //             2 => surf_en_change -= ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change - self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 1;
    //             self.front.tpa_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) < 6 {
    //                 self.front.tpb_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => self.front.tpa_add(neib_idx),
    //                     1 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 0)
    //                         {
    //                             self.front.tpb_rem(neib_idx);
    //                         }
    //                     }
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Add");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     if is_rem_step {
    //         let tpb_len = self.front.tpbs_size;
    //         let idxl = rng.random_range(0..tpb_len);
    //         let idxg = self.front.tpbs[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change -= ex2,
    //             2 => surf_en_change += ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change -= ey2,
    //             2 => surf_en_change += ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change -= ez2,
    //             2 => surf_en_change += ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change + self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 0;
    //             self.front.tpb_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) > 0 {
    //                 self.front.tpa_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 1)
    //                         {
    //                             self.front.tpa_rem(neib_idx);
    //                         }
    //                     }
    //                     1 => self.front.tpb_add(neib_idx),
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Rem");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     if self.simlog.p_b > rng.random::<f64>() {
    //         let tpb_len = self.front.tpbs_size;
    //         let idxl = rng.random_range(0..tpb_len);
    //         let idxg = self.front.tpbs[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change -= ex2,
    //             2 => surf_en_change += ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change -= ey2,
    //             2 => surf_en_change += ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change -= ez2,
    //             2 => surf_en_change += ez2,
    //             _ => {}
    //         }
    //         // d_e = surf_en_change + self.simlog.dg.val;

    //         self.simlog.add_denergy(surf_en_change);

    //         self.state[idxg] = 0;
    //         self.front.tpb_rem(idxg);
    //         if (smx_yz + smy_xz + smz_xy) > 0 {
    //             self.front.tpa_add(idxg);
    //         }

    //         let mut has_invalid_neib = false;

    //         for &neib_idx in idxg_nis.iter() {
    //             if neib_idx == usize::MAX {
    //                 has_invalid_neib = true;
    //                 continue;
    //             }

    //             match self.state[neib_idx] {
    //                 0 => {
    //                     if !neibs[neib_idx]
    //                         .iter()
    //                         .any(|&n| n != usize::MAX && self.state[n] == 1)
    //                     {
    //                         self.front.tpa_rem(neib_idx);
    //                     }
    //                 }
    //                 1 => self.front.tpb_add(neib_idx),
    //                 _ => {}
    //             }
    //         }

    //         if has_invalid_neib {
    //             self.handle_stalled_boundary(step_id);
    //             return self.is_alive;
    //         }

    //         if self.is_front_empty() {
    //             self.handle_stalled_front(step_id, "Ballistic Rem");
    //             return self.is_alive;
    //         }
    //     }

    //     self.simlog.mk_step.val = step_id;

    //     if is_write_step {
    //         self.write_action(grid);
    //     }

    //     self.is_alive
    // }

    // pub fn mode_1_3_step(
    //     &mut self,
    //     step_id: u64,
    //     (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    //     rng: &mut ChaCha8Rng,
    //     neibs: &[[usize; 6]],
    //     grid: &mut Grid,
    //     (ex2, ey2, ez2, eisol): (f64, f64, f64, f64),
    // ) -> bool {
    //     let (mut surf_en_change, mut d_e) = (0.0, 0.0);

    //     if is_add_step {
    //         let tpa_len = self.front.tpas_size;
    //         let idxl = rng.random_range(0..tpa_len);
    //         let idxg = self.front.tpas[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change += ex2,
    //             2 => surf_en_change -= ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change += ey2,
    //             2 => surf_en_change -= ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change += ez2,
    //             2 => surf_en_change -= ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change - self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 1;
    //             self.front.tpa_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) < 6 {
    //                 self.front.tpb_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => self.front.tpa_add(neib_idx),
    //                     1 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 0)
    //                         {
    //                             self.front.tpb_rem(neib_idx);
    //                         }
    //                     }
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Add");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     if is_rem_step {
    //         let tpb_len = self.front.tpbs_size;
    //         let idxl = rng.random_range(0..tpb_len);
    //         let idxg = self.front.tpbs[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change -= ex2,
    //             2 => surf_en_change += ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change -= ey2,
    //             2 => surf_en_change += ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change -= ez2,
    //             2 => surf_en_change += ez2,
    //             _ => {}
    //         }
    //         d_e = surf_en_change + self.simlog.dg.val;

    //         if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 0;
    //             self.front.tpb_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) > 0 {
    //                 self.front.tpa_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 1)
    //                         {
    //                             self.front.tpa_rem(neib_idx);
    //                         }
    //                     }
    //                     1 => self.front.tpb_add(neib_idx),
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Rem");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     'ballistic_rem: {
    //         let tpb_len = self.front.tpbs_size;
    //         let idxl = rng.random_range(0..tpb_len);
    //         let idxg = self.front.tpbs[idxl];
    //         let idxg_nis = &neibs[idxg];
    //         let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

    //         surf_en_change = 0.0;
    //         match smx_yz {
    //             0 => surf_en_change -= ex2,
    //             2 => surf_en_change += ex2,
    //             _ => {}
    //         }
    //         match smy_xz {
    //             0 => surf_en_change -= ey2,
    //             2 => surf_en_change += ey2,
    //             _ => {}
    //         }
    //         match smz_xy {
    //             0 => surf_en_change -= ez2,
    //             2 => surf_en_change += ez2,
    //             _ => {}
    //         }
    //         // d_e = surf_en_change + self.simlog.dg.val;

    //         let prob =
    //             self.simlog.p_b * (1.0f64 - (surf_en_change / eisol)).powf(self.simlog.p_pow);
    //         if prob > rng.random::<f64>() {
    //             self.simlog.add_denergy(surf_en_change);

    //             self.state[idxg] = 0;
    //             self.front.tpb_rem(idxg);
    //             if (smx_yz + smy_xz + smz_xy) > 0 {
    //                 self.front.tpa_add(idxg);
    //             }

    //             let mut has_invalid_neib = false;

    //             for &neib_idx in idxg_nis.iter() {
    //                 if neib_idx == usize::MAX {
    //                     has_invalid_neib = true;
    //                     continue;
    //                 }

    //                 match self.state[neib_idx] {
    //                     0 => {
    //                         if !neibs[neib_idx]
    //                             .iter()
    //                             .any(|&n| n != usize::MAX && self.state[n] == 1)
    //                         {
    //                             self.front.tpa_rem(neib_idx);
    //                         }
    //                     }
    //                     1 => self.front.tpb_add(neib_idx),
    //                     _ => {}
    //                 }
    //             }

    //             if has_invalid_neib {
    //                 self.handle_stalled_boundary(step_id);
    //                 return self.is_alive;
    //             }

    //             if self.is_front_empty() {
    //                 self.handle_stalled_front(step_id, "Rem");
    //                 return self.is_alive;
    //             }
    //         }
    //     }

    //     self.simlog.mk_step.val = step_id;

    //     if is_write_step {
    //         self.write_action(grid);
    //     }

    //     self.is_alive
    // }

    pub fn mode_2_1_step(
        &mut self,
        rng: &mut ChaCha8Rng,
        grid: &mut Grid,
        (ex2, ey2, ez2): (f64, f64, f64),
        step_id: u64,
        (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    ) -> bool {
        let neibs = &*grid.neibs;

        let (mut surf_en_change, mut d_e) = (0.0, 0.0);

        if is_add_step {
            let tpa_len = self.front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = self.front.tpas[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change - self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 1;
                self.front.tpa_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) < 6 {
                    self.front.tpb_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => self.front.tpa_add(neib_idx),
                        1 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 0)
                            {
                                self.front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Add");
                    return self.is_alive;
                }
            }
        }

        if is_rem_step {
            let tpb_len = self.front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = self.front.tpbs[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change + self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(-1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 0;
                self.front.tpb_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) > 0 {
                    self.front.tpa_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 1)
                            {
                                self.front.tpa_rem(neib_idx);
                            }
                        }
                        1 => self.front.tpb_add(neib_idx),
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Rem");
                    return self.is_alive;
                }
            }
        }

        self.simlog.mk_step.val = step_id;

        if is_write_step {
            self.write_action(grid);
        }

        self.is_alive
    }

    pub fn mode_2_2_step(
        &mut self,
        rng: &mut ChaCha8Rng,
        grid: &mut Grid,
        (ex2, ey2, ez2): (f64, f64, f64),
        step_id: u64,
        (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    ) -> bool {
        let neibs = &*grid.neibs;

        let (mut surf_en_change, mut d_e) = (0.0, 0.0);

        if is_add_step {
            let tpa_len = self.front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = self.front.tpas[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change - self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 1;
                self.front.tpa_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) < 6 {
                    self.front.tpb_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => self.front.tpa_add(neib_idx),
                        1 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 0)
                            {
                                self.front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Add");
                    return self.is_alive;
                }
            }
        }

        if is_rem_step {
            let tpb_len = self.front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = self.front.tpbs[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change + self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(-1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 0;
                self.front.tpb_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) > 0 {
                    self.front.tpa_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 1)
                            {
                                self.front.tpa_rem(neib_idx);
                            }
                        }
                        1 => self.front.tpb_add(neib_idx),
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Rem");
                    return self.is_alive;
                }
            }
        }

        if self.simlog.p_b > rng.random::<f64>() {
            let tpb_len = self.front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = self.front.tpbs[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            // d_e = surf_en_change + self.simlog.dg.val;

            self.simlog.update_n_sizes(-1.0);
            self.simlog.update_conc();
            self.simlog.add_denergy(surf_en_change);

            self.state[idxg] = 0;
            self.front.tpb_rem(idxg);
            if (smx_yz + smy_xz + smz_xy) > 0 {
                self.front.tpa_add(idxg);
            }

            let mut has_invalid_neib = false;

            for &neib_idx in idxg_nis.iter() {
                if neib_idx == usize::MAX {
                    has_invalid_neib = true;
                    continue;
                }

                match self.state[neib_idx] {
                    0 => {
                        if !neibs[neib_idx]
                            .iter()
                            .any(|&n| n != usize::MAX && self.state[n] == 1)
                        {
                            self.front.tpa_rem(neib_idx);
                        }
                    }
                    1 => self.front.tpb_add(neib_idx),
                    _ => {}
                }
            }

            if has_invalid_neib {
                self.handle_stalled_boundary(step_id);
                return self.is_alive;
            }

            if self.is_front_empty() {
                self.handle_stalled_front(step_id, "Ballistic Rem");
                return self.is_alive;
            }
        }

        self.simlog.mk_step.val = step_id;

        if is_write_step {
            self.write_action(grid);
        }

        self.is_alive
    }

    pub fn mode_2_3_step(
        &mut self,
        rng: &mut ChaCha8Rng,
        grid: &mut Grid,
        (ex2, ey2, ez2, eisol): (f64, f64, f64, f64),
        step_id: u64,
        (is_add_step, is_rem_step, is_write_step): (bool, bool, bool),
    ) -> bool {
        let neibs = &*grid.neibs;

        let (mut surf_en_change, mut d_e) = (0.0, 0.0);

        if is_add_step {
            let tpa_len = self.front.tpas_size;
            let idxl = rng.random_range(0..tpa_len);
            let idxg = self.front.tpas[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change - self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 1;
                self.front.tpa_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) < 6 {
                    self.front.tpb_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => self.front.tpa_add(neib_idx),
                        1 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 0)
                            {
                                self.front.tpb_rem(neib_idx);
                            }
                        }
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Add");
                    return self.is_alive;
                }
            }
        }

        if is_rem_step {
            let tpb_len = self.front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = self.front.tpbs[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            d_e = surf_en_change + self.simlog.dg.val;

            if d_e < 0.0 || (-d_e / self.simlog.k_t).exp() > rng.random::<f64>() {
                self.simlog.update_n_sizes(-1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 0;
                self.front.tpb_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) > 0 {
                    self.front.tpa_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 1)
                            {
                                self.front.tpa_rem(neib_idx);
                            }
                        }
                        1 => self.front.tpb_add(neib_idx),
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Rem");
                    return self.is_alive;
                }
            }
        }

        'ballistic_rem: {
            let tpb_len = self.front.tpbs_size;
            let idxl = rng.random_range(0..tpb_len);
            let idxg = self.front.tpbs[idxl];
            let idxg_nis = &neibs[idxg];
            let (smx_yz, smy_xz, smz_xy) = compute_neighbor_sums(&self.state, idxg_nis);

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
            // d_e = surf_en_change + self.simlog.dg.val;

            let prob =
                self.simlog.p_b * (1.0f64 - (surf_en_change / eisol)).powf(self.simlog.p_pow);
            if prob > rng.random::<f64>() {
                self.simlog.update_n_sizes(-1.0);
                self.simlog.update_conc();
                self.simlog.add_denergy(surf_en_change);

                self.state[idxg] = 0;
                self.front.tpb_rem(idxg);
                if (smx_yz + smy_xz + smz_xy) > 0 {
                    self.front.tpa_add(idxg);
                }

                let mut has_invalid_neib = false;

                for &neib_idx in idxg_nis.iter() {
                    if neib_idx == usize::MAX {
                        has_invalid_neib = true;
                        continue;
                    }

                    match self.state[neib_idx] {
                        0 => {
                            if !neibs[neib_idx]
                                .iter()
                                .any(|&n| n != usize::MAX && self.state[n] == 1)
                            {
                                self.front.tpa_rem(neib_idx);
                            }
                        }
                        1 => self.front.tpb_add(neib_idx),
                        _ => {}
                    }
                }

                if has_invalid_neib {
                    self.handle_stalled_boundary(step_id);
                    return self.is_alive;
                }

                if self.is_front_empty() {
                    self.handle_stalled_front(step_id, "Rem");
                    return self.is_alive;
                }
            }
        }

        self.simlog.mk_step.val = step_id;

        if is_write_step {
            self.write_action(grid);
        }

        self.is_alive
    }
}
