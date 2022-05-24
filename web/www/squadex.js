var GRAVITY = 9.8;
var VELOCITY = 109.890938;

function rad_to_deg(rad) {
	return rad * 180.0 / Math.PI;
}

function milliradians_from_meters(meters, alt_delta = 0) {
	var p1 = Math.sqrt(VELOCITY ** 4 - GRAVITY * (GRAVITY * meters ** 2 + 2 * alt_delta * VELOCITY ** 2));
	var a1 = Math.atan((VELOCITY ** 2 + p1) / (GRAVITY * meters));
	return rad_to_deg(a1) / (360.0 / 6400.0);
}