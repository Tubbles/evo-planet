//! Planet parameters and the derived quantities that feed both the physics
//! simulation (gravity, fluid densities, oxygen) and the abstract ecology
//! readouts (productivity, plant:animal ratio, expected lung size).
//!
//! The ecology formulas are deliberately simple, Earth-normalised models.
//! They are meant to be legible dials for a game, not climate science.

use serde::{Deserialize, Serialize};

pub const EARTH_GRAVITY: f32 = 9.81;
pub const EARTH_AIR_DENSITY: f32 = 1.225; // kg/m^3 at sea level, 15 C
pub const EARTH_PO2_KPA: f32 = 21.2; // partial pressure of O2 at sea level
pub const WATER_DENSITY: f32 = 1000.0;
pub const R_GAS: f32 = 8.314; // J/(mol K)

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanetParams {
    pub name: String,
    /// Surface gravity, m/s^2.
    pub gravity: f32,
    /// Surface pressure in Earth atmospheres.
    pub pressure_atm: f32,
    /// Molar fraction of O2 in the atmosphere (Earth 0.21).
    pub o2_frac: f32,
    /// Molar fraction of CO2 (Earth 0.0004). The remainder is treated as N2.
    pub co2_frac: f32,
    /// Mean surface temperature, degrees C.
    pub temp_c: f32,
    /// Fraction of surface covered by liquid water (Earth 0.71).
    pub water_coverage: f32,
    /// Relative humidity of land air, 0..1 (Earth ~0.6 average).
    pub humidity: f32,
    /// Stellar flux relative to Earth (1.0 = Earth).
    pub insolation: f32,
}

impl Default for PlanetParams {
    fn default() -> Self {
        Self::earth()
    }
}

impl PlanetParams {
    pub fn earth() -> Self {
        Self {
            name: "Earth".into(),
            gravity: 9.81,
            pressure_atm: 1.0,
            o2_frac: 0.21,
            co2_frac: 0.0004,
            temp_c: 15.0,
            water_coverage: 0.71,
            humidity: 0.6,
            insolation: 1.0,
        }
    }

    pub fn presets() -> Vec<PlanetParams> {
        vec![
            Self::earth(),
            Self {
                name: "Thin Mars-like".into(),
                gravity: 3.7,
                pressure_atm: 0.1,
                o2_frac: 0.15,
                co2_frac: 0.05,
                temp_c: -5.0,
                water_coverage: 0.15,
                humidity: 0.2,
                insolation: 0.45,
            },
            Self {
                name: "Dense low-g (flyer's world)".into(),
                gravity: 5.5,
                pressure_atm: 2.5,
                o2_frac: 0.25,
                co2_frac: 0.001,
                temp_c: 22.0,
                water_coverage: 0.5,
                humidity: 0.7,
                insolation: 0.9,
            },
            Self {
                name: "Super-Earth".into(),
                gravity: 17.0,
                pressure_atm: 1.4,
                o2_frac: 0.19,
                co2_frac: 0.002,
                temp_c: 18.0,
                water_coverage: 0.6,
                humidity: 0.55,
                insolation: 1.1,
            },
            Self {
                name: "Ocean world".into(),
                gravity: 8.0,
                pressure_atm: 1.1,
                o2_frac: 0.23,
                co2_frac: 0.0008,
                temp_c: 24.0,
                water_coverage: 0.97,
                humidity: 0.95,
                insolation: 1.05,
            },
            Self {
                name: "Hot arid".into(),
                gravity: 10.5,
                pressure_atm: 0.9,
                o2_frac: 0.18,
                co2_frac: 0.004,
                temp_c: 42.0,
                water_coverage: 0.2,
                humidity: 0.12,
                insolation: 1.5,
            },
            Self {
                name: "Low-O2 swamp".into(),
                gravity: 9.0,
                pressure_atm: 1.0,
                o2_frac: 0.09,
                co2_frac: 0.01,
                temp_c: 28.0,
                water_coverage: 0.8,
                humidity: 0.9,
                insolation: 1.0,
            },
        ]
    }

    pub fn derive(&self) -> Derived {
        let t_k = self.temp_c + 273.15;
        let n2_frac = (1.0 - self.o2_frac - self.co2_frac).max(0.0);
        // Mean molar mass in kg/mol.
        let molar_mass = self.o2_frac * 0.032 + self.co2_frac * 0.044 + n2_frac * 0.028;
        let pressure_pa = self.pressure_atm * 101_325.0;
        let air_density = pressure_pa * molar_mass / (R_GAS * t_k);
        let po2_kpa = self.pressure_atm * 101.325 * self.o2_frac;
        let o2_rel = po2_kpa / EARTH_PO2_KPA;

        // --- Primary productivity, Earth-normalised -------------------------
        // Light: saturating. CO2: Michaelis-Menten with half-saturation near
        // 250 ppm (C3 plants are CO2-limited at Earth levels). Temperature:
        // Gaussian around 22 C with width 16 C. Water: humidity and coverage.
        let light = 1.0 - (-self.insolation * 1.6).exp();
        let light_earth = 1.0 - (-1.6f32).exp();
        let co2_ppm = self.co2_frac * 1.0e6;
        let co2_fac = co2_ppm / (co2_ppm + 250.0);
        let co2_fac_earth = 400.0 / (400.0 + 250.0);
        let temp_fac = (-((self.temp_c - 22.0) / 16.0).powi(2)).exp();
        let temp_fac_earth = (-((15.0f32 - 22.0) / 16.0).powi(2)).exp();
        let water_fac = 0.4 * self.humidity + 0.6 * self.water_coverage.sqrt();
        let water_fac_earth = 0.4 * 0.6 + 0.6 * 0.71f32.sqrt();
        let npp_rel = (light / light_earth)
            * (co2_fac / co2_fac_earth)
            * (temp_fac / temp_fac_earth)
            * (water_fac / water_fac_earth);

        // --- Animal cost model -----------------------------------------------
        // Terrestrial locomotion cost scales with weight; aquatic cost is
        // buoyancy-relieved. Oxygen availability caps sustainable metabolic
        // rate: below Earth pO2, animals must be smaller or slower.
        let g_rel = self.gravity / EARTH_GRAVITY;
        let land_cost = 0.35 + 0.65 * g_rel;
        let sea_cost = 0.75 + 0.1 * g_rel;
        let aquatic_share = self.water_coverage;
        let mean_cost = aquatic_share * sea_cost + (1.0 - aquatic_share) * land_cost;
        let o2_cap = o2_rel.clamp(0.05, 3.0).powf(0.6);
        // Earth: roughly 200 kg of plant carbon per kg of animal carbon.
        let plant_animal_ratio = 200.0 * mean_cost / o2_cap;
        let animal_biomass_rel = npp_rel * o2_cap / mean_cost;

        // Lungs: human lungs are ~8% of body volume at Earth pO2. Demand grows
        // with gravity-driven cost; supply with pO2 and diffusion.
        let lung_frac = (0.08 * land_cost / o2_rel.max(0.02)).clamp(0.01, 0.6);
        let gill_frac = (0.04 * sea_cost / o2_rel.max(0.02)).clamp(0.005, 0.4);

        // Water storage need: aridity index on land.
        let aridity = ((1.0 - self.humidity) * (0.5 + 0.5 * (self.temp_c / 30.0).max(0.0)))
            .clamp(0.0, 1.5);
        // Earth-average humidity (aridity ~0.3) needs almost none; deserts need a lot.
        let water_storage_frac = (0.02 + 0.4 * (aridity - 0.25).max(0.0)).clamp(0.0, 0.5);

        // Flight: stall speed for a reference wing loading of 50 N/m^2 with a
        // flat-plate Cl_max of about 1.2. Flight index compares how much the
        // atmosphere helps versus how much gravity hurts.
        let cl_max = 1.2;
        let wing_loading_ref = 50.0;
        let stall_speed_ref = (2.0 * wing_loading_ref / (air_density * cl_max)).sqrt();
        let flight_index = (air_density / EARTH_AIR_DENSITY) / g_rel;
        // Wing loading a glider can carry at 8 m/s.
        let wing_loading_at_8ms = 0.5 * air_density * 64.0 * cl_max;

        Derived {
            air_density,
            water_density: WATER_DENSITY,
            po2_kpa,
            o2_rel,
            molar_mass,
            npp_rel,
            plant_animal_ratio,
            animal_biomass_rel,
            lung_frac,
            gill_frac,
            water_storage_frac,
            aridity,
            stall_speed_ref,
            flight_index,
            wing_loading_at_8ms,
            aquatic_share,
            land_cost,
            sea_cost,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Derived {
    pub air_density: f32,
    pub water_density: f32,
    pub po2_kpa: f32,
    pub o2_rel: f32,
    pub molar_mass: f32,
    pub npp_rel: f32,
    pub plant_animal_ratio: f32,
    pub animal_biomass_rel: f32,
    /// Expected lung volume fraction for an active land animal.
    pub lung_frac: f32,
    /// Expected gill fraction for an aquatic animal.
    pub gill_frac: f32,
    /// Expected water-storage mass fraction for a land animal.
    pub water_storage_frac: f32,
    pub aridity: f32,
    /// Stall speed (m/s) at wing loading 50 N/m^2.
    pub stall_speed_ref: f32,
    /// (air density / Earth) / (gravity / Earth). >1 means flight is easier than Earth.
    pub flight_index: f32,
    /// Wing loading (N/m^2) supportable at 8 m/s glide speed.
    pub wing_loading_at_8ms: f32,
    pub aquatic_share: f32,
    pub land_cost: f32,
    pub sea_cost: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_is_normalised() {
        let d = PlanetParams::earth().derive();
        assert!((d.air_density - 1.22).abs() < 0.03, "{}", d.air_density);
        assert!((d.o2_rel - 1.0).abs() < 0.02);
        assert!((d.npp_rel - 1.0).abs() < 1e-3);
        assert!((d.flight_index - 1.0).abs() < 0.03);
        assert!((d.lung_frac - 0.08).abs() < 0.01);
    }

    #[test]
    fn thin_air_hurts_flight_and_grows_lungs() {
        let mars = &PlanetParams::presets()[1];
        let d = mars.derive();
        let e = PlanetParams::earth().derive();
        assert!(d.flight_index < e.flight_index);
        assert!(d.lung_frac > e.lung_frac);
        assert!(d.plant_animal_ratio > 0.0);
    }
}
