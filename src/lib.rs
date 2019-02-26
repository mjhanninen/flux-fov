use std::f32;

////////////////////////////////////////////////////////////////////////////////

/// A pre-computed flux field.
///
/// The flux field is used by the field of vision algorithm to determine how
/// the rays emanating from a single grid cell "flow" outwards to the
/// surrounding cells.
///
pub struct FluxField {
    radius: usize,
    flux_lut: Vec<f32>,
}

impl FluxField {
    /// Constructs a flux field covering the area within `radius`.
    ///
    pub fn new(radius: usize) -> Self {
        FluxField {
            radius,
            // The ray radius and count are just the first numbers I came
            // up with.
            flux_lut: calc_flux_lut(radius, 100 * radius, 10_000),
        }
    }
}

// The construction of the look-up table for the flux field is somewhat
// tricky.  However when trying to understand it, keep the following diagram
// in you mind.
//
//   y 5...../
//     4..../j
//     3.../fi
//     2../ceh
//     1./abdg
//     0@-----
//      012345 x
//
// The horizontal and diagonal edges flanking the octant are special cases and
// we don't need a look-up table for them.  So we ignore them.
//
// For the interior we want to build up a lookup table of flux weights of the
// form:
//
//     abcdefghij
//     0123456789
//
// This way the the look-up table has the flux weights in the same order as
// the order of visiting the interior cells while updating the
// field-of-vision.

#[derive(Clone, Default)]
struct RayCount {
    jump: i32,
    total: i32,
}

fn calc_flux_lut(flux_field_radius: usize, ray_radius: usize, ray_count: usize) -> Vec<f32> {
    assert!(ray_count > 1);
    assert!(flux_field_radius > 0);
    assert!(ray_radius as f32 / flux_field_radius as f32 >= f32::consts::SQRT_2);
    let ray_radius = ray_radius as f32;
    let counts_wd = flux_field_radius - 1;
    let counts_size = counts_wd * counts_wd;
    let mut counts: Vec<RayCount> = vec![Default::default(); counts_size];
    for ray_ix in 0..ray_count {
        let ray_angle = ray_ix as f32 / (ray_count - 1) as f32 * f32::consts::FRAC_PI_4;
        let target_x = (ray_angle.cos() * ray_radius).round() as usize;
        let target_y = (ray_angle.sin() * ray_radius).round() as usize;
        let mut last_y = 0;
        march_ray(flux_field_radius, target_x, target_y, |x, y| {
            if 1 < x && 0 < y && y < x {
                let ix = (y - 1) * counts_wd + x - 2;
                let ray_count = &mut counts[ix];
                ray_count.total += 1;
                if last_y != y {
                    ray_count.jump += 1;
                }
            }
            last_y = y;
        });
    }
    let lut_size = (flux_field_radius - 1) * flux_field_radius / 2;
    let mut lut = Vec::with_capacity(lut_size);
    for x in 0..(flux_field_radius - 1) {
        for y in 0..(x + 1) {
            let ray_count = &counts[y * counts_wd + x];
            lut.push(ray_count.jump as f32 / ray_count.total as f32);
        }
    }
    lut
}

// March a ray from the origin to the direction of the point (`target_x`,
// `target_y`) calling the function `f` at every point along the march.  The
// march is stopped once the x-coordinate has reached `limit_x`.
//
fn march_ray<F>(limit_x: usize, target_x: usize, target_y: usize, f: F)
where
    F: FnMut(usize, usize),
{
    assert!(target_y <= target_x, "illegal arguments");
    let mut f = f;
    if target_y == 0 {
        for step_x in 0..limit_x + 1 {
            f(step_x, 0);
        }
    } else if target_y == target_x {
        for step_x in 0..limit_x + 1 {
            f(step_x, step_x);
        }
    } else {
        f(0, 0);
        let mut r = target_x / 2;
        let mut step_y = 0;
        for step_x in 1..limit_x + 1 {
            r += target_y;
            if r >= target_x {
                step_y += 1;
                r -= target_x;
            }
            f(step_x, step_y);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A field of vision.
///
pub struct Fov<T, X>
where
    X: AsRef<FluxField>,
{
    flux_field: X,
    radius: isize,
    width: isize,
    ix_origin: isize,
    data: Vec<T>,
}

impl<T, X> Fov<T, X>
where
    T: Clone,
    X: AsRef<FluxField>,
{
    pub fn new(flux_field: X, radius: usize, init: T) -> Self {
        assert!(radius <= flux_field.as_ref().radius);
        let radius = radius as isize;
        let width = radius * 2 + 1;
        let ix_origin = radius * (width + 1);
        let size = (width * width) as usize;
        let data = vec![init; size];
        Fov {
            flux_field,
            radius,
            width,
            ix_origin,
            data,
        }
    }
}

/// An influx into a grid cell.
///
pub struct Influx<T> {
    pub weight: f32,
    pub dx: i32,
    pub dy: i32,
    pub value: T,
}

impl<T, X> Fov<T, X>
where
    X: AsRef<FluxField>,
{
    /// The value of at the given grid cell.
    pub fn at(&self, x: i32, y: i32) -> &T {
        let ix = self.ix_origin + (self.width * y as isize) + x as isize;
        &self.data[ix as usize]
    }

    /// Expose the underlying data as a slice.
    pub fn as_slice(&self) -> &[T] {
        self.data.as_slice()
    }

    /// Update the field of vision with the given function.
    ///
    pub fn update<F>(&mut self, update_fn: F)
    where
        F: FnMut(i32, i32, &[Influx<&T>]) -> T,
    {
        // The field of view is laid out in the memory in the following
        // manner:
        //
        //         <----W---->
        //         <-R-> <-R->
        //
        //     ^ ^ \6666|7777/
        //     | | 5\666|777/8            -y
        //     | R 55\66|77/88
        //     | | 555\6|7/888            |
        //     | v 5555\|/8888            |
        //     W   -----@-----       -x --@-- +x
        //     | ^ 4444/|\1111            |
        //     | | 444/3|2\111            |
        //     | R 44/33|22\11
        //     | | 4/333|222\1            +y
        //     v v /3333|2222\
        //
        // Here R is the "radius" of the field of view and W is the width
        // of a single row (W = 2 * R + 1).

        unsafe {
            let mut h = Helper::new(self, update_fn);
            let w = h.width;
            h.calc_origin();
            if h.radius > 0 {
                h.calc_edge(1, 0, 1);
                h.calc_edge(1, 1, w + 1);
                h.calc_edge(0, 1, w);
                h.calc_edge(-1, 1, w - 1);
                h.calc_edge(-1, 0, -1);
                h.calc_edge(-1, -1, -w - 1);
                h.calc_edge(0, -1, -w);
                h.calc_edge(1, -1, -w + 1);
                if h.radius > 1 {
                    h.calc_interior(1, 0, 0, 1, 1, w);
                    h.calc_interior(0, 1, 1, 0, w, 1);
                    h.calc_interior(0, -1, 1, 0, w, -1);
                    h.calc_interior(-1, 0, 0, 1, -1, w);
                    h.calc_interior(-1, 0, 0, -1, -1, -w);
                    h.calc_interior(0, -1, -1, 0, -w, -1);
                    h.calc_interior(0, 1, -1, 0, -w, 1);
                    h.calc_interior(1, 0, 0, -1, 1, -w);
                }
            }
        }
    }
}

struct Helper<'a, T, F> {
    update_fn: F,
    origin: *mut T,
    radius: isize,
    width: isize,
    flux_lut: &'a [f32],
}

impl<'a, T, F> Helper<'a, T, F>
where
    T: Sized,
    F: FnMut(i32, i32, &[Influx<&T>]) -> T,
{
    #[inline]
    unsafe fn new<X>(fov: &'a mut Fov<T, X>, update_fn: F) -> Self
    where
        X: AsRef<FluxField>,
    {
        Helper {
            update_fn,
            origin: fov.data.as_mut_ptr().offset(fov.ix_origin),
            radius: fov.radius,
            width: fov.width,
            flux_lut: &fov.flux_field.as_ref().flux_lut,
        }
    }

    #[inline]
    unsafe fn calc_origin(&mut self) {
        *self.origin = (self.update_fn)(0, 0, &[]);
    }

    #[inline]
    unsafe fn calc_edge(&mut self, dx: i32, dy: i32, stride: isize) {
        let mut x = 0;
        let mut y = 0;
        let mut curr = self.origin;
        for _ in 0..self.radius {
            x += dx;
            y += dy;
            let prev = &*curr;
            curr = curr.offset(stride);
            *curr = (self.update_fn)(
                x,
                y,
                &[Influx {
                    dx,
                    dy,
                    weight: 1.0,
                    value: prev,
                }],
            );
        }
    }

    #[inline]
    unsafe fn calc_interior(
        &mut self,
        m_xu: i32,
        m_xv: i32,
        m_yu: i32,
        m_yv: i32,
        u_stride: isize,
        v_stride: isize,
    ) {
        assert!(self.radius > 1);
        let dx_stay = -m_xu;
        let dy_stay = -m_yu;
        let dx_jump = -(m_xu + m_xv);
        let dy_jump = -(m_yu + m_yv);
        let mut col_ptr = self.origin.offset(u_stride);
        let mut lut_ix = 0;
        for u in 2..self.radius as i32 + 1 {
            let mut influx_ptr = col_ptr;
            let mut influx_stay = &*influx_ptr;
            col_ptr = col_ptr.offset(u_stride);
            let mut curr = col_ptr;
            for v in 1..u {
                curr = curr.offset(v_stride);
                influx_ptr = influx_ptr.offset(v_stride);
                let influx_jump = &*influx_ptr;
                let x = m_xu * u + m_xv * v;
                let y = m_yu * u + m_yv * v;
                let w = self.flux_lut[lut_ix];
                *curr = (self.update_fn)(
                    x,
                    y,
                    &[
                        Influx {
                            dx: dx_stay,
                            dy: dy_stay,
                            // XXX(soija) Hmm, IMO this should be 1.0 - w
                            // and the other one w ... but this the way
                            // they give the correct result.  Need to
                            // figure this out.
                            weight: w,
                            value: influx_stay,
                        },
                        Influx {
                            dx: dx_jump,
                            dy: dy_jump,
                            weight: 1.0 - w,
                            value: influx_jump,
                        },
                    ],
                );
                influx_stay = influx_jump;
                lut_ix += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::io::{self, Write};

    fn coordinate_flag(sz: i32) -> String {
        let flux_field = Box::new(FluxField::new(sz as usize));
        let mut fov = Fov::new(flux_field, sz as usize, (-1, -1));
        fov.update(|x, y, _| (x, y));
        let mut w = io::Cursor::new(Vec::new());
        let mut ix = 0;
        for y in -sz..sz + 1 {
            if y > -sz {
                write!(w, "; ").unwrap();
            }
            for x in -sz..sz + 1 {
                if x > -sz {
                    write!(w, "  ").unwrap();
                }
                let p = fov.as_slice()[ix];
                write!(w, "{:+}{:+}", p.0, p.1).unwrap();
                ix += 1;
            }
        }
        String::from_utf8(w.into_inner()).unwrap()
    }

    #[test]
    fn test_1() {
        assert_eq!(coordinate_flag(0), "+0+0");
    }

    #[test]
    fn small_coordinate_flag() {
        assert_eq!(
            coordinate_flag(1),
            "-1-1  +0-1  +1-1; \
             -1+0  +0+0  +1+0; \
             -1+1  +0+1  +1+1"
        );
    }

    #[test]
    fn big_coordinate_flag() {
        assert_eq!(
            coordinate_flag(5),
            "-5-5  -4-5  -3-5  -2-5  -1-5  +0-5  +1-5  +2-5  +3-5  +4-5  +5-5; \
             -5-4  -4-4  -3-4  -2-4  -1-4  +0-4  +1-4  +2-4  +3-4  +4-4  +5-4; \
             -5-3  -4-3  -3-3  -2-3  -1-3  +0-3  +1-3  +2-3  +3-3  +4-3  +5-3; \
             -5-2  -4-2  -3-2  -2-2  -1-2  +0-2  +1-2  +2-2  +3-2  +4-2  +5-2; \
             -5-1  -4-1  -3-1  -2-1  -1-1  +0-1  +1-1  +2-1  +3-1  +4-1  +5-1; \
             -5+0  -4+0  -3+0  -2+0  -1+0  +0+0  +1+0  +2+0  +3+0  +4+0  +5+0; \
             -5+1  -4+1  -3+1  -2+1  -1+1  +0+1  +1+1  +2+1  +3+1  +4+1  +5+1; \
             -5+2  -4+2  -3+2  -2+2  -1+2  +0+2  +1+2  +2+2  +3+2  +4+2  +5+2; \
             -5+3  -4+3  -3+3  -2+3  -1+3  +0+3  +1+3  +2+3  +3+3  +4+3  +5+3; \
             -5+4  -4+4  -3+4  -2+4  -1+4  +0+4  +1+4  +2+4  +3+4  +4+4  +5+4; \
             -5+5  -4+5  -3+5  -2+5  -1+5  +0+5  +1+5  +2+5  +3+5  +4+5  +5+5"
        );
    }

    fn flux_connection_flag(sz: i32) -> String {
        let flux_field = Box::new(FluxField::new(sz as usize));
        let mut fov = Fov::new(flux_field, sz as usize, -1);
        fov.update(|x, y, influxes| {
            if x == 0 && y == 0 {
                1_i32
            } else {
                influxes.iter().map(|f| *f.value).sum()
            }
        });
        let mut w = io::Cursor::new(Vec::new());
        let mut ix = 0;
        for _ in -sz..sz + 1 {
            write!(w, "[  ").unwrap();
            for x in -sz..sz + 1 {
                if x > -sz {
                    write!(w, "  ").unwrap();
                }
                write!(w, "{:2}", fov.as_slice()[ix]).unwrap();
                ix += 1;
            }
            write!(w, "  ] ").unwrap();
        }
        String::from_utf8(w.into_inner()).unwrap()
    }

    #[test]
    fn null_connection_flag() {
        assert_eq!(flux_connection_flag(1), "[   1  ] ");
    }

    #[test]
    fn small_connection_flag() {
        assert_eq!(
            flux_connection_flag(1),
            "[   1   1   1  ] \
             [   1   1   1  ] \
             [   1   1   1  ] "
        );
    }

    #[test]
    fn big_connection_flag() {
        assert_eq!(
            flux_connection_flag(5),
            "[   1   5  10  10   5   1   5  10  10   5   1  ] \
             [   5   1   4   6   4   1   4   6   4   1   5  ] \
             [  10   4   1   3   3   1   3   3   1   4  10  ] \
             [  10   6   3   1   2   1   2   1   3   6  10  ] \
             [   5   4   3   2   1   1   1   2   3   4   5  ] \
             [   1   1   1   1   1   1   1   1   1   1   1  ] \
             [   5   4   3   2   1   1   1   2   3   4   5  ] \
             [  10   6   3   1   2   1   2   1   3   6  10  ] \
             [  10   4   1   3   3   1   3   3   1   4  10  ] \
             [   5   1   4   6   4   1   4   6   4   1   5  ] \
             [   1   5  10  10   5   1   5  10  10   5   1  ] "
        );
    }

    fn weight_flag(sz: i32) -> String {
        let flux_field = Box::new(FluxField::new(sz as usize));
        let mut fov = Fov::new(flux_field, sz as usize, -1.0);
        fov.update(|_, _, influxes| influxes.iter().map(|f| f.weight).sum());
        let mut w = io::Cursor::new(Vec::new());
        let mut ix = 0;
        for _ in -sz..sz + 1 {
            write!(w, "[  ").unwrap();
            for x in -sz..sz + 1 {
                if x > -sz {
                    write!(w, "  ").unwrap();
                }
                write!(w, "{:.3}", fov.as_slice()[ix]).unwrap();
                ix += 1;
            }
            write!(w, "  ] ").unwrap();
        }
        String::from_utf8(w.into_inner()).unwrap()
    }

    #[test]
    fn small_weight_flag() {
        assert_eq!(
            weight_flag(1),
            "[  1.000  1.000  1.000  ] \
             [  1.000  0.000  1.000  ] \
             [  1.000  1.000  1.000  ] "
        );
    }

    #[test]
    fn big_weight_flag() {
        assert_eq!(
            weight_flag(5),
            "[  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  0.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] \
             [  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  1.000  ] "
        );
    }
}
