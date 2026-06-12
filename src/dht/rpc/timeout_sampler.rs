use std::fmt;

#[derive(Clone)]
pub struct Snapshot {
    values: Vec<f32>,
    mean: f32,
    mode: f32,
    bin_size: u32,
    max_bin: u32,
}

impl Snapshot {
    pub fn new(mut bins: Vec<f32>, bin_size: u32, max_bin: u32) -> Self {
        Self::normalize(&mut bins);

        let mut snapshot = Self {
            values: bins,
            mean: 0.0,
            mode: 0.0,
            bin_size,
            max_bin,
        };

        snapshot.calc_stats();
        snapshot
    }

    fn normalize(values: &mut [f32]) {
        let total: f32 = values.iter().sum();

        if total > 0.0 {
            for v in values {
                *v /= total;
            }
        }
    }

    fn calc_stats(&mut self) {
        let mut mode_pop = 0.0;

        for (bin, value) in self.values.iter().enumerate() {
            let midpoint = (bin as f32 + 0.5) * self.bin_size as f32;

            self.mean += value * midpoint;

            if *value > mode_pop {
                self.mode = midpoint;
                mode_pop = *value;
            }
        }
    }

    pub fn quantile(&self, mut quant: f32) -> f32 {
        for (idx, value) in self.values.iter().enumerate() {
            quant -= value;

            if quant <= 0.0 {
                return (idx as f32 + 0.5) * self.bin_size as f32;
            }
        }

        self.max_bin as f32
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Statistics: mean={}, median={}, mode={}, 10tile={}, 90tile={}",
            self.mean,
            self.quantile(0.5),
            self.mode,
            self.quantile(0.1),
            self.quantile(0.9)
        )?;

        for (i, value) in self.values.iter().enumerate() {
            if *value >= 0.001 {
                write!(f, " {:5} | ", i as u32 * self.bin_size)?;
            }
        }

        writeln!(f)?;

        for value in &self.values {
            if *value >= 0.001 {
                write!(f, "{:5}‰ | ", (value * 1000.0).round() as u32)?;
            }
        }

        Ok(())
    }
}

#[allow(unused)]
pub struct TimeoutSampler {
    timeout_min: u64,
    timeout_max: u64,
    timeout_baseline_min: u64,

    min_bin: u64,
    max_bin: u64,
    bin_size: u64,

    bins: Vec<f32>,
    update_count: u64,

    timeout_ceiling: u64,
    timeout_baseline: u64,

    snapshot: Snapshot,
}

#[allow(unused)]
impl TimeoutSampler {
    pub fn new(
        bin_size: u64,
        timeout_min: u64,
        timeout_max: u64,
        timeout_baseline_min: u64,
    ) -> Self {
        assert!(bin_size > 0);
        assert!(timeout_min < timeout_max);

        let num_bins =
            ((timeout_max - timeout_min) as f64 / bin_size as f64).ceil() as usize;

        let bins = vec![0.0; num_bins];

        let mut snapshot_bins = bins.clone();
        snapshot_bins[num_bins - 1] = 1.0;

        let snapshot = Snapshot::new(
            snapshot_bins,
            bin_size as u32,
            timeout_max as u32,
        );

        let mut sampler = Self {
            timeout_min,
            timeout_max,
            timeout_baseline_min,

            min_bin: timeout_min,
            max_bin: timeout_max,
            bin_size,

            bins,
            update_count: 0,

            timeout_ceiling: timeout_max,
            timeout_baseline: timeout_max,

            snapshot,
        };

        sampler.reset();
        sampler
    }

    pub fn reset(&mut self) {
        self.update_count = 0;
        self.timeout_baseline = self.timeout_max;
        self.timeout_ceiling = self.timeout_max;

        let value = 1.0f32 / self.bins.len() as f32;

        self.bins.fill(value);
    }

    pub fn sample_count(&self) -> u64 {
        self.update_count
    }

    pub fn update(&mut self, rtt: u64) {
        let mut bin =
            ((rtt.saturating_sub(self.min_bin)) / self.bin_size) as usize;

        if bin >= self.bins.len() {
            bin = self.bins.len() - 1;
        }

        self.bins[bin] += 1.0;
    }

    pub fn update_and_recalc(&mut self, rtt: u64) {
        self.update(rtt);

        if (self.update_count & 0x0f) == 0 {
            self.make_snapshot();
            self.decay();
        }

        self.update_count += 1;
    }

    pub fn decay(&mut self) {
        for bin in &mut self.bins {
            *bin *= 0.95;
        }
    }

    pub fn make_snapshot(&mut self) {
        self.snapshot = Snapshot::new(
            self.bins.clone(),
            self.bin_size as u32,
            self.max_bin as u32,
        );

        self.timeout_baseline =
            self.snapshot.quantile(0.1) as u64;

        self.timeout_ceiling =
            self.snapshot.quantile(0.9) as u64;
    }

    pub fn stats(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn stall_timeout(&self) -> u64 {
        let timeout = std::cmp::max(
            self.timeout_baseline + self.timeout_baseline_min,
            self.timeout_ceiling,
        );

        timeout
            .min(self.timeout_max)
            .max(self.timeout_min)
    }
}
