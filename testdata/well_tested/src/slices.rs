use std::borrow::Cow;

fn pad<'a>(aa: &'a mut [Cow<'static, str>]) -> &'a [Cow<'static, str>] {
    for a in aa.iter_mut() {
        if a.len() < 3 {
            a.to_mut().push_str("___");
        }
    }
    aa
}

fn return_mut_slice(a: &mut [usize]) -> &mut [usize] {
    for x in a.iter_mut() {
        *x *= 2
    }
    a
}

#[cfg(test)]
mod test {
    #[test]
    fn test_pad() {
        assert_eq!(
            super::pad(&mut ["hello".into(), "ok".into(), "cat".into()]),
            ["hello", "ok___", "cat"]
        );
    }

    #[test]
    fn mut_slice() {
        assert_eq!(super::return_mut_slice(&mut [1, 2, 3]), [2, 4, 6]);
    }
}
