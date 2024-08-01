
#[macro_export]
macro_rules! make_static {
    ($t:ty, $val:expr) => ($crate::make_static!($t, $val,));
    ($t:ty, $val:expr, $(#[$m:meta])*) => {{
        $(#[$m])*
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}