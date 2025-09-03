use crate::mods::{
    constants::K_BOLTZMANN,
    frontier::Frontier,
    io_handler,
    lattice::Grid,
    settings::{Settings, SettingsError},
    state::SimLog,
};

#[inline(always)]
pub fn rebuild_front(states: &[u8], neibs: &[[usize; 6]], front: &mut Frontier) -> f64 {
    let mut cluster_size = 0.0;

    println!("Updating gas and cluster fronts...");
    for (i, &state) in states.iter().enumerate() {
        if state == 1 {
            cluster_size += 1.0;

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
        "Update completed! Gas front nodes: {}, Cluster front nodes: {}",
        front.tpas_size, front.tpbs_size,
    );

    cluster_size
}

#[inline(always)]
pub fn compute_neighbor_sums(states: &[u8], idxg_nis: &[usize; 6]) -> (u8, u8, u8) {
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

// #[inline(always)]
// pub fn compute_neighbor_sums(states: &[u8], idxg_nis: [usize; 6]) -> (bool, bool, u8, u8, u8) {
//     let mut has_crystal_neib = false;
//     let mut has_gas_neib = false;
//     let mut x_axis_neighbors = 0;
//     let mut y_axis_neighbors = 0;
//     let mut z_axis_neighbors = 0;

//     for i in 0..6 {
//         let idx = unsafe { *idxg_nis.get_unchecked(i) };
//         if idx != usize::MAX {
//             let state = unsafe { *states.get_unchecked(idx) };

//             match state {
//                 0 => {}
//                 1 => {
//                     match i {
//                         0 | 1 => x_axis_neighbors += 1,
//                         2 | 3 => y_axis_neighbors += 1,
//                         4 | 5 => z_axis_neighbors += 1,
//                         _ => unreachable!(), // Цикл всегда от 0 до 5
//                     }
//                 }
//                 _ => {}
//             }
//         }
//     }

//     (
//         has_crystal_neib,
//         has_gas_neib,
//         x_axis_neighbors,
//         y_axis_neighbors,
//         z_axis_neighbors,
//     )
// }

// #[inline(always)]
// pub fn calc_front(&mut self, neibs: &[[usize; 6]]) -> (usize, usize) {
//     let states = &*self.state;
//     let mut front_tpa = vec![0; states.len()].into_boxed_slice();
//     let mut front_tpb = vec![0; states.len()].into_boxed_slice();

//     let (mut has_crystal_neib, mut has_gas_neib) = (false, false);

//     for (idxg, &state) in states.iter().enumerate() {
//         match state {
//             0 => {
//                 has_crystal_neib = false;

//                 'neibs_loop: for neib_idxg in neibs[idxg] {
//                     if neib_idxg != usize::MAX {
//                         if states[neib_idxg] == 1 {
//                             has_crystal_neib = true;
//                             break 'neibs_loop;
//                         }
//                     }
//                 }

//                 if has_crystal_neib {
//                     front_tpa[idxg] = 1
//                 }
//             }
//             1 => {
//                 has_gas_neib = false;

//                 'neibs_loop: for neib_idxg in neibs[idxg] {
//                     if neib_idxg != usize::MAX {
//                         if states[neib_idxg] == 0 {
//                             has_gas_neib = true;
//                             break 'neibs_loop;
//                         }
//                     }
//                 }

//                 if has_gas_neib {
//                     front_tpb[idxg] = 1
//                 }
//             }
//             _ => {}
//         }
//     }

//     let tpas_size: usize = front_tpa.iter().sum();
//     let tpbs_size: usize = front_tpb.iter().sum();

//     (tpas_size, tpbs_size)
// }
