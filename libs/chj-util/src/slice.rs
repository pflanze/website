
pub fn first<T>(items: &[T]) -> Option<&T> {
    if items.len() > 0 {
        Some(&items[0])
    } else {
        None
    }
}

pub fn rest<T>(items: &[T]) -> Option<&[T]> {
    if items.len() > 0 {
        Some(&items[1..])
    } else {
        None
    }
}

pub fn first_and_rest<T>(items: &[T]) -> Option<(&T, &[T])> {
    if items.len() > 0 {
        Some((&items[0], &items[1..]))
    } else {
        None
    }
}


