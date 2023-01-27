mod command;

fn main() {
    let output = command::safe_command("ps -e --no-header -o pid,user:30,pcpu,pmem,comm", 2);
    println!("{output:?}");
}
