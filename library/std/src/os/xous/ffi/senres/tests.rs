#[test]
fn smoke_test() {
    use super::Stack;
    let mut sr1 = Stack::<4096>::new();
    // let sr2 = Senres::<4097>::new();
    let sr3 = Stack::<8192>::new();
    // let sr4 = Senres::<4098>::new();
    let sr5 = Stack::new();

    do_stuff(&sr5);
    println!("Size of sr1: {}", core::mem::size_of_val(&sr1));
    println!("Size of sr3: {}", core::mem::size_of_val(&sr3));
    println!("Size of sr5: {}", core::mem::size_of_val(&sr5));

    {
        let mut writer = sr1.writer(*b"test").unwrap();
        writer.append(16777215u32);
        writer.append(u64::MAX);
        writer.append("Hello, world!");
        writer.append("String2");
        writer.append::<Option<u32>>(None);
        writer.append::<Option<u32>>(Some(42));
        writer.append(96u8);
        writer.append([1i32, 2, 3, 4, 5].as_slice());
        writer.append([5u8, 4, 3, 2].as_slice());
        writer.append([5u16, 4, 2]);
        writer.append(["Hi", "There", "123456789"]);
        // writer.append(["Hello".to_owned(), "There".to_owned(), "World".to_owned()].as_slice());
    }
    // println!("sr1: {:?}", sr1);

    {
        let reader = sr1.reader(*b"test").expect("couldn't get reader");
        let val: u32 = reader.try_get_from().expect("couldn't get the u32 value");
        println!("u32 val: {}", val);
        let val: u64 = reader.try_get_from().expect("couldn't get the u64 value");
        println!("u64 val: {:x}", val);
        let val: &str = reader.try_get_ref_from().expect("couldn't get string value");
        println!("String val: {}", val);
        let val: String = reader.try_get_from().expect("couldn't get string2 value");
        println!("String2 val: {}", val);
        let val: Option<u32> = reader.try_get_from().expect("couldn't get Option<u32>");
        println!("Option<u32> val: {:?}", val);
        let val: Option<u32> = reader.try_get_from().expect("couldn't get Option<u32>");
        println!("Option<u32> val: {:?}", val);

        let val: u8 = reader.try_get_from().expect("couldn't get u8 weird padding");
        println!("u8 val: {}", val);

        let val: &[i32] = reader.try_get_ref_from().expect("couldn't get &[i32]");
        println!("&[i32] val: {:?}", val);
        let val: &[u8] = reader.try_get_ref_from().expect("couldn't get &[u8]");
        println!("&[u8] val: {:?}", val);
        let val: [u16; 3] = reader.try_get_from().expect("couldn't get [u16; 3]");
        println!("[u16; 3] val: {:?}", val);
        let val: [String; 3] = reader.try_get_from().expect("couldn't get [String; 3]");
        println!("[String; 3] val: {:?}", val);
    }
}
