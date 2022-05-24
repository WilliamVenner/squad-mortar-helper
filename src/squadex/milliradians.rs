const GRAVITY: f64 = 9.8;
const VELOCITY: f64 = 109.890938;

#[inline]
pub fn calc(meters: f64, alt_delta: f64) -> f64 {
	let p1 = f64::sqrt(VELOCITY.powi(4) - GRAVITY * (GRAVITY * meters.powi(2) + 2.0 * alt_delta * VELOCITY.powi(2)));
	let a1 = f64::atan((VELOCITY.powi(2) + p1) / (GRAVITY * meters));
	a1.to_degrees() / (360.0 / 6400.0)
}