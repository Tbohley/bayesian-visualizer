use super::*;

#[test]
fn parses_an_embedded_csv_dataset() {
    let dataset = Dataset::from_csv("example.csv", b"x,y\n1.5,2\n3,4.25\n").unwrap();

    assert_eq!(dataset.name, "example.csv");
    assert_eq!(dataset.n, 2);
    assert_eq!(dataset.data["x"], vec![1.5, 3.0]);
    assert_eq!(dataset.data["y"], vec![2.0, 4.25]);
}
