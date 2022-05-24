use crate::prelude::SusRefCell;
use imgui::sys::ImVec2;

static ROTATION_START_INDEX: SusRefCell<i32> = SusRefCell::new(0);

#[inline]
fn im_rotate(v: &ImVec2, sin_a: f32, cos_a: f32) -> ImVec2 {
	ImVec2 {
		x: v.x * cos_a - v.y * sin_a,
		y: v.x * sin_a + v.y * cos_a
	}
}

fn im_rotation_center() -> ImVec2 {
	// ImVec2 l(FLT_MAX, FLT_MAX), u(-FLT_MAX, -FLT_MAX); // bounds
	let mut l = ImVec2 {
		x: f32::MAX,
		y: f32::MAX
	};

	let mut u = ImVec2 {
		x: -f32::MAX,
		y: -f32::MAX
	};

	#[inline]
	fn im_min_vec2(lhs: &ImVec2, rhs: &ImVec2) -> ImVec2 {
		ImVec2 {
			x: if lhs.x < rhs.x { lhs.x } else { rhs.x },
			y: if lhs.y < rhs.y { lhs.y } else { rhs.y }
		}
	}

	#[inline]
	fn im_max_vec2(lhs: &ImVec2, rhs: &ImVec2) -> ImVec2 {
		ImVec2 {
			x: if lhs.x >= rhs.x { lhs.x } else { rhs.x },
			y: if lhs.y >= rhs.y { lhs.y } else { rhs.y }
		}
	}

	unsafe {
		let buf = &mut (*imgui::sys::igGetWindowDrawList()).VtxBuffer;
		for i in *ROTATION_START_INDEX.borrow()..buf.Size {
			let vert = *buf.Data.offset(i as isize);
			l = im_min_vec2(&l, &vert.pos);
			u = im_max_vec2(&u, &vert.pos);
		}
	}

	ImVec2 {
		x: (l.x + u.x) / 2.0,
		y: (l.y + u.y) / 2.0
	}
}

pub trait ImRotate {
	fn rotate(&self, rad: f32, center: Option<[f32; 2]>, draw_list: super::DrawList) -> ImRotateHandle;
}
impl ImRotate for imgui::Ui<'_> {
	fn rotate(&self, rad: f32, center: Option<[f32; 2]>, draw_list: super::DrawList) -> ImRotateHandle {
		*ROTATION_START_INDEX.borrow_mut() = unsafe { (*draw_list.as_ptr()).VtxBuffer.Size };

		ImRotateHandle {
			rad,
			center: center.map(|[x, y]| ImVec2 { x, y }),
			draw_list
		}
	}
}

pub struct ImRotateHandle {
	rad: f32,
	center: Option<ImVec2>,
	draw_list: super::DrawList
}
impl ImRotateHandle {
	pub fn end(self) {}
}
impl Drop for ImRotateHandle {
    fn drop(&mut self) {
		let s = self.rad.sin();
		let c = self.rad.cos();

		let center = self.center.unwrap_or_else(im_rotation_center);
		let center = {
			let lhs = im_rotate(&center, s, c);
			ImVec2 {
				x: lhs.x - center.x,
				y: lhs.y - center.y
			}
		};

		unsafe {
			let buf = &mut (*self.draw_list.as_ptr()).VtxBuffer;
			for i in *ROTATION_START_INDEX.borrow()..buf.Size {
				let vert = &mut *buf.Data.offset(i as isize);
				vert.pos = {
					let lhs = im_rotate(&vert.pos, s, c);
					ImVec2 {
						x: lhs.x - center.x,
						y: lhs.y - center.y
					}
				};
			}
		}
    }
}