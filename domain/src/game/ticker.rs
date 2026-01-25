use rand::Rng;

#[derive(Clone, Debug)]
pub struct Ticker {
    base_volatility: i32,
    base_pressure: i32,
    forces: Vec<MarketForce>,
}

impl Ticker {
    pub fn new(base_volatility: i32) -> Self {
        Self {
            base_volatility,
            base_pressure: 0,
            forces: Vec::new(),
        }
    }

    pub fn next_delta(&self) -> i32 {
        let mut rng = rand::thread_rng();
        let conditions = self.compute_conditions();

        let effective_volatility = self.base_volatility + (conditions.volatility * self.base_volatility as f32) as i32;
        let effective_pressure = self.base_pressure + (conditions.pressure * self.base_volatility as f32) as i32;

        rng.gen_range(-effective_volatility..=effective_volatility) + effective_pressure
    }

    pub fn add_force(
        &mut self,
        pressure: f32,
        volatility: f32,
        decay: Decay,
    ) {
        self.forces.push(MarketForce::new(pressure, volatility, decay));
    }

    pub fn compute_conditions(&self) -> MarketConditions {
        let mut conditions = MarketConditions::default();
        for force in &self.forces {
            conditions.pressure += force.effective_pressure();
            conditions.volatility += force.effective_volatility();
        }
        conditions
    }

    pub fn tick(&mut self) {
        for force in &mut self.forces {
            force.decay.tick();
        }
        self.forces.retain(|f| f.decay.strength() > 0.0);
    }

    pub fn on_bid_placed(
        &mut self,
        bid_value: f32,
    ) {
        // Fast bullish
        self.add_force(bid_value / 800.0, 0.0, Decay::linear(5));
        // Slow bearish reversion (90% of fast total)
        self.add_force(-bid_value / 3000.0, 0.0, Decay::linear(20));
    }

    pub fn on_ask_placed(
        &mut self,
        ask_value: f32,
    ) {
        // Fast bearish spike
        self.add_force(-ask_value / 800.0, 0.0, Decay::linear(5));
        // Slow bullish reversion (90% of fast total)
        self.add_force(ask_value / 3000.0, 0.0, Decay::linear(20));
    }

    pub fn on_bid_filled(
        &mut self,
        filled_at: f32,
    ) {
        // Fast bearish (demand consumed) + volatility spike
        self.add_force(-filled_at / 1000.0, 0.08, Decay::linear(4));
        // Slow bullish reversion (full reversion - fills are completed transactions)
        self.add_force(filled_at / 2640.0, 0.0, Decay::linear(18));
    }

    pub fn on_ask_filled(
        &mut self,
        filled_at: f32,
    ) {
        // Fast bullish (supply consumed) + volatility spike
        self.add_force(filled_at / 1000.0, 0.08, Decay::linear(4));
        // Slow bearish reversion (full reversion - fills are completed transactions)
        self.add_force(-filled_at / 2640.0, 0.0, Decay::linear(18));
    }
}

#[derive(Clone, Debug, Default)]
pub struct MarketConditions {
    pub pressure: f32,
    pub volatility: f32,
}

#[derive(Clone, Debug)]
pub enum Decay {
    Instant,
    Duration { remaining: u32 },
    Linear { remaining: u32, initial: u32 },
    Exponential { half_life: f32, age: f32 },
}

impl Decay {
    pub fn duration(ticks: u32) -> Self {
        Decay::Duration { remaining: ticks }
    }

    pub fn linear(ticks: u32) -> Self {
        Decay::Linear {
            remaining: ticks,
            initial: ticks,
        }
    }

    pub fn exponential(half_life: f32) -> Self {
        Decay::Exponential { half_life, age: 0.0 }
    }

    pub fn strength(&self) -> f32 {
        match self {
            Decay::Instant => 1.0,
            Decay::Duration { remaining } => {
                if *remaining > 0 {
                    1.0
                } else {
                    0.0
                }
            }
            Decay::Linear { remaining, initial } => {
                if *initial == 0 {
                    0.0
                } else {
                    *remaining as f32 / *initial as f32
                }
            }
            Decay::Exponential { half_life, age } => {
                if *half_life <= 0.0 {
                    0.0
                } else {
                    0.5_f32.powf(*age / *half_life)
                }
            }
        }
    }

    pub fn tick(&mut self) -> bool {
        match self {
            Decay::Instant => false,
            Decay::Duration { remaining } => {
                *remaining = remaining.saturating_sub(1);
                *remaining > 0
            }
            Decay::Linear { remaining, .. } => {
                *remaining = remaining.saturating_sub(1);
                *remaining > 0
            }
            Decay::Exponential { age, .. } => {
                *age += 1.0;
                self.strength() > 0.01
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarketForce {
    pub pressure: f32,
    pub volatility: f32,
    pub decay: Decay,
}

impl MarketForce {
    pub fn new(
        pressure: f32,
        volatility: f32,
        decay: Decay,
    ) -> Self {
        Self {
            pressure,
            volatility,
            decay,
        }
    }

    pub fn effective_pressure(&self) -> f32 {
        self.pressure * self.decay.strength()
    }

    pub fn effective_volatility(&self) -> f32 {
        self.volatility * self.decay.strength()
    }
}

#[derive(Clone, Debug)]
pub struct PlayerTicker {
    pub ticker: Ticker,
    pub current_price: i32,
}

impl PlayerTicker {
    pub fn new(
        base_volatility: i32,
        starting_price: i32,
    ) -> Self {
        Self {
            ticker: Ticker::new(base_volatility),
            current_price: starting_price,
        }
    }

    pub fn tick(&mut self) {
        self.ticker.tick();
        self.current_price = (self.current_price + self.ticker.next_delta()).max(0);
    }
}
