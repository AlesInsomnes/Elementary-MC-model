// mod mods;

// use mods::{
//     constants::INIT_TIME_STATES_FILE_NAME, ensemble::Ensemble, frontier::Frontier, io_handler,
//     lattice::Grid, settings::Settings, simulation::run_calculations,
// };

// use std::{fs::File, io::BufWriter, time::Instant};

// use rand::SeedableRng;
// use rand_chacha::ChaCha8Rng;

mod mods;

use mods::{
    constants::INIT_TIME_STATES_FILE_NAME, ensemble::Ensemble, frontier::Frontier, io_handler,
    lattice::Grid, settings::Settings,
};

use std::{fs::File, io::BufWriter, time::Instant};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sta1 = Instant::now();

    let mut ensemble = match Ensemble::new() {
        Ok(e) => {
            println!("✅ Ensemble created successfully!");
            e
        }
        Err(e) => {
            eprintln!("❌ Failed to create ensemble: {}", e);
            std::process::exit(1);
        }
    };

    let _ = ensemble.run_simulation();

    // println!("src_path: {:?}", ensemble.cfg.src_path);

    // let sta2 = Instant::now();

    // let mut cfg = Settings::new();
    // match io_handler::load_config(&mut cfg).and_then(|_| Ok(cfg.validate())) {
    //     Ok(_) => { /* println!("✅ Settings loaded and validated!") */ }
    //     Err(e) => {
    //         eprintln!("❌ Error: {}", e);
    //         std::process::exit(1)
    //     }
    // }

    // let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);

    // let mut grid = Grid::new(cfg.sx, cfg.sy, cfg.sz, cfg.px, cfg.py, cfg.pz);

    // let mut front = Frontier::new(grid.size);

    // io_handler::prepare_dir(&mut cfg).unwrap_or_else(|e| {
    //     eprintln!("❌ Failed to create output directory: {}", e);
    //     std::process::exit(1);
    // });
    // // println!("📁 SRC Path: {}", cfg.src_path.display());
    // println!("📁 DST Path: {}", cfg.dst_path.display());

    // let path_dst_states = io_handler::prepare_files(&mut cfg).unwrap_or_else(|e| {
    //     eprintln!("❌ Failed to prepare files: {}", e);
    //     std::process::exit(1);
    // });
    // // println!("States file Path: {}", path_dst_states.display());

    // let dst_states = File::create(path_dst_states)?;
    // let mut dst_states_buf = BufWriter::new(dst_states);

    // let states = match io_handler::load_states(&cfg) {
    //     Ok(data) => data, // Если загрузка успешна, получаем Vec<Vec<u8>>
    //     Err(e) => {
    //         // Если произошла ошибка, выводим сообщение и завершаем программу
    //         eprintln!("❌ Failed to load states: {}", e);
    //         std::process::exit(1);
    //     }
    // };

    // let fin2 = sta2.elapsed();
    // println!("✅ Preparation DONE! (Time: {:?})", fin2);

    // println!("DirPrefix: {:?}", cfg.dir_prefix);
    // println!("Seed: {:?};", cfg.seed);
    // println!("Sx: {:?}; Sy: {:?}; Sz: {:?};", cfg.sx, cfg.sy, cfg.sz);
    // println!("Px: {:?}; Py: {:?}; Pz: {:?};", cfg.px, cfg.py, cfg.pz);
    // println!("T: {:.5e};", cfg.temperature);
    // println!(
    //     "Ax: {:.5e}; Ay: {:.5e}; Az: {:.5e};",
    //     cfg.ax, cfg.ay, cfg.az
    // );
    // println!(
    //     "g100: {:.5e}; g010: {:.5e}; g001: {:.5e};",
    //     cfg.g100, cfg.g010, cfg.g001
    // );
    // println!("mode: {:?}; dg: {:.5e};", cfg.mode, cfg.dg);
    // println!(
    //     "C_eq: {:.5e}; C0: {:.5e}; N_tot: {:.5e}; N0_cr: {:.5e}; p_b: {:.5e}; p_pow: {:.5e};",
    //     cfg.c_eq, cfg.c0, cfg.n_tot, cfg.n0_cr, cfg.p_b, cfg.p_pow
    // );
    // println!(
    //     "AddI: {:?}; AddFrom: {:?}; RemI: {:?}; RemFrom: {:?};",
    //     cfg.add_i, cfg.add_from, cfg.rem_i, cfg.rem_from,
    // );
    // println!("LoadOption: {:?};", cfg.load_option);
    // println!(
    //     "StepLim: {:?}; PrintI: {:?};  WriteI: {:?};",
    //     cfg.step_lim, cfg.print_i, cfg.write_i,
    // );

    // run_calculations(&cfg, &mut grid, &mut front, &mut rng, &mut dst_states_buf)?;

    let fin1 = sta1.elapsed();
    println!("✅ All DONE! (Time: {:?})", fin1);

    Ok(())
}
