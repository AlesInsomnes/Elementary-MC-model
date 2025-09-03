#[derive(Debug)]
pub struct Grid {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub size: usize,
    pub size_xy: usize,
    pub size_zx: usize,
    pub size_zy: usize,
    pub px: bool,
    pub py: bool,
    pub pz: bool,
    // pub states: Box<[u8]>,
    pub nx_ib: Box<[usize]>,
    pub ny_ib: Box<[usize]>,
    pub nz_ib: Box<[usize]>,
    pub neibs: Box<[[usize; 6]]>,
}

impl Grid {
    #[inline(always)]
    pub fn new(nx: usize, ny: usize, nz: usize, px: bool, py: bool, pz: bool) -> Self {
        let size = nx * ny * nz;
        let mut grid = Grid {
            nx,
            ny,
            nz,
            size,
            size_xy: nx * ny,
            size_zx: nz * nx,
            size_zy: nz * ny,
            px,
            py,
            pz,
            // states: vec![0u8; size].into_boxed_slice(),
            nx_ib: vec![0; nx].into_boxed_slice(),
            ny_ib: vec![0; ny].into_boxed_slice(),
            nz_ib: vec![0; nz].into_boxed_slice(),
            neibs: vec![[usize::MAX; 6]; size].into_boxed_slice(),
        };
        grid.precomp_neibs();
        grid
    }

    #[inline(always)]
    pub fn xyz_to_idx(&self, x: usize, y: usize, z: usize) -> usize {
        z + y * self.nz + x * self.size_zy
    }

    #[inline(always)]
    pub fn idx_to_xyz(&self, idx: usize) -> (usize, usize, usize) {
        let z = idx % self.nz;
        let y = (idx / self.nz) % self.ny;
        let x = idx / self.size_zy;
        (x, y, z)
    }

    #[inline(always)]
    fn xyz_to_periodic_sub(coord: isize, dim_size: usize, periodic: bool) -> usize {
        let dim = dim_size as isize;
        if (0..dim).contains(&coord) {
            coord as usize
        } else if periodic {
            coord.rem_euclid(dim) as usize
        } else {
            usize::MAX
        }
    }

    #[inline(always)]
    pub fn xyz_to_periodic(&self, x: isize, y: isize, z: isize) -> (usize, usize, usize) {
        (
            Self::xyz_to_periodic_sub(x, self.nx, self.px),
            Self::xyz_to_periodic_sub(y, self.ny, self.py),
            Self::xyz_to_periodic_sub(z, self.nz, self.pz),
        )
    }

    #[inline(always)]
    pub fn precomp_neibs(&mut self) {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (px, py, pz) = (self.px, self.py, self.pz);

        for idx in 0..self.size {
            let (x, y, z) = self.idx_to_xyz(idx);
            let x = x as isize;
            let y = y as isize;
            let z = z as isize;

            let neighbors = [
                (x - 1, y, z),
                (x + 1, y, z),
                (x, y - 1, z),
                (x, y + 1, z),
                (x, y, z - 1),
                (x, y, z + 1),
            ];

            let neibs_entry = &mut self.neibs[idx];

            for (i, &(xi, yi, zi)) in neighbors.iter().enumerate() {
                let xpi = Self::xyz_to_periodic_sub(xi, nx, px);
                let ypi = Self::xyz_to_periodic_sub(yi, ny, py);
                let zpi = Self::xyz_to_periodic_sub(zi, nz, pz);

                neibs_entry[i] = if xpi != usize::MAX && ypi != usize::MAX && zpi != usize::MAX {
                    zpi + ypi * self.nz + xpi * self.size_zy
                } else {
                    usize::MAX
                };
            }
        }
    }
}
