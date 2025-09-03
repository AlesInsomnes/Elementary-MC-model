use std::cmp::max;

#[derive(Debug)]
pub struct Frontier {
    pub tpas: Vec<usize>,
    pub tpbs: Vec<usize>,
    idxg_to_type: Box<[u8]>,
    idxg_to_idxl: Box<[usize]>,
    pub tpas_size: usize,
    pub tpbs_size: usize,
}

impl Frontier {
    #[inline(always)]
    pub fn new(total_grid_size: usize) -> Self {
        let initial_capacity = max(total_grid_size / 10, 128);
        Self {
            tpas: Vec::with_capacity(initial_capacity),
            tpbs: Vec::with_capacity(initial_capacity),
            idxg_to_type: vec![0; total_grid_size].into_boxed_slice(),
            idxg_to_idxl: vec![0; total_grid_size].into_boxed_slice(),
            tpas_size: 0,
            tpbs_size: 0,
        }
    }

    #[inline(always)]
    pub fn tpa_add(&mut self, idxg: usize) {
        if self.idxg_to_type[idxg] == 2 {
            return;
        }

        self.idxg_to_idxl[idxg] = self.tpas_size;
        self.tpas.push(idxg);
        self.idxg_to_type[idxg] = 2;
        self.tpas_size += 1;
    }

    #[inline(always)]
    pub fn tpa_rem(&mut self, idxg: usize) {
        if self.idxg_to_type[idxg] != 2 {
            return;
        }

        self.idxg_to_type[idxg] = 0;
        let idxl = self.idxg_to_idxl[idxg];
        self.tpas_size -= 1;

        let last_idxg = self
            .tpas
            .pop()
            .unwrap();

        if idxl != self.tpas_size {
            self.tpas[idxl] = last_idxg;
            self.idxg_to_idxl[last_idxg] = idxl;
        }
        self.idxg_to_idxl[idxg] = 0;
    }

    #[inline(always)]
    pub fn tpb_add(&mut self, idxg: usize) {
        if self.idxg_to_type[idxg] == 3 {
            return;
        }
        self.idxg_to_idxl[idxg] = self.tpbs_size;
        self.tpbs.push(idxg);
        self.idxg_to_type[idxg] = 3;
        self.tpbs_size += 1;
    }

    #[inline(always)]
    pub fn tpb_rem(&mut self, idxg: usize) {
        if self.idxg_to_type[idxg] != 3 {
            return;
        }
        self.idxg_to_type[idxg] = 0;
        let idxl = self.idxg_to_idxl[idxg];
        self.tpbs_size -= 1;
        let last_idxg = self
            .tpbs
            .pop()
            .unwrap();

        if idxl != self.tpbs_size {
            self.tpbs[idxl] = last_idxg;
            self.idxg_to_idxl[last_idxg] = idxl;
        }
        self.idxg_to_idxl[idxg] = 0;
    }
}
