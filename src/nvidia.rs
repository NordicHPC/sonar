// For prototyping purposes (and maybe it's good enough for production?), parse the output of
// `nvidia-smi pmon`.  This output has a couple of problems:
//
//  - it is (documented to be) not necessarily stable
//  - it does not orphaned processes holding onto GPU memory, the way nvtop can do
//
// To fix the latter problem we do something with --query-compute-apps, see later.
//
// TODO: We could consider using the underlying C library instead, but this adds a fair
// amount of complexity.  See the nvidia-smi manual page.
//
// TODO: Maybe #ifdef all this NVIDIA stuff on a build config that is NVIDIA-specific?

const NVIDIA_PMON_COMMAND: &str = "nvidia-smi pmon -c 1 -s mu";

// Returns (user-name, pid, command-name) -> (device-mask, gpu-util-pct, gpu-mem-pct, gpu-mem-size-in-kib)
// where the values are summed across all devices and the device-mask is a bitmask for the
// GPU devices used by that process.  For a system with 8 cards, utilization
// can reach 800% and the memory size can reach the sum of the memories on the cards.

fn extract_nvidia_pmon_processes(
    raw_text: &str,
    user_by_pid: &HashMap<String, String>,
) -> HashMap<(String, String, String), (u32, f64, f64, usize)> {
    let result = raw_text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let (_start_indices, parts) = chunks(line);
            let device = parts[0].parse::<usize>().unwrap();
            let pid = parts[1];
            let maybe_mem_usage = parts[3].parse::<usize>();
            let maybe_gpu_pct = parts[4].parse::<f64>();
            let maybe_mem_pct = parts[5].parse::<f64>();
            // For nvidia-smi, we use the first word because the command produces
            // blank-padded output.  We can maybe do better by considering non-empty words.
            let command = parts[8].to_string();
            let user = match user_by_pid.get(pid) {
                Some(name) => name.clone(),
                None => "_zombie_".to_owned() + pid,
            };
            (
                pid,
                device,
                user,
                maybe_mem_usage,
                maybe_gpu_pct,
                maybe_mem_pct,
                command,
            )
        })
        .filter(|(pid, ..)| *pid != "-")
        .map(
            |(pid, device, user, maybe_mem_usage, maybe_gpu_pct, maybe_mem_pct, command)| {
                (
                    (user, pid.to_string(), command),
                    (
                        1 << device,
                        maybe_gpu_pct.unwrap_or(0.0),
                        maybe_mem_pct.unwrap_or(0.0),
                        maybe_mem_usage.unwrap_or(0usize) * 1024,
                    ),
                )
            },
        )
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((device, gpu_pct, mem_pct, mem_size)) = acc.get_mut(&key) {
                *device |= value.0;
                *gpu_pct += value.1;
                *mem_pct += value.2;
                *mem_size += value.3;
            } else {
                acc.insert(key, value);
            }
            acc
        });
    result
}

// We use this to get information about processes that are not captured by pmon.  It's hacky
// but it works.

const NVIDIA_QUERY_COMMAND: &str =
    "nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits";

// Same signature as extract_nvidia_pmon_processes(), q.v. but user is always "_zombie_" and command
// is always "_unknown_".  Only pids not in user_by_pid are returned.

fn extract_nvidia_query_processes(
    raw_text: &str,
    user_by_pid: &HashMap<String, String>,
) -> HashMap<(String, String, String), (u32, f64, f64, usize)> {
    let result = raw_text
        .lines()
        .map(|line| {
            let (_start_indices, parts) = chunks(line);
            let pid = parts[0].strip_suffix(',').unwrap();
            let mem_usage = parts[1].parse::<usize>().unwrap();
            let user = "_zombie_".to_owned() + pid;
            let command = "_unknown_";
            (
                (user, pid.to_string(), command.to_string()),
                (!0, 0.0, 0.0, mem_usage * 1024),
            )
        })
        .filter(|((_, pid, _), _)| !user_by_pid.contains_key(pid))
        .fold(HashMap::new(), |mut acc, (key, value)| {
            if let Some((device, _gpu_pct, _mem_pct, mem_size)) = acc.get_mut(&key) {
                *device |= value.0;
                *mem_size += value.3;
            } else {
                acc.insert(key, value);
            }
            acc
        });
    result
}

// Shared test cases for the NVIDIA stuff

#[cfg(test)]
mod test_nvidia {
    use super::*;

    fn mkusers() -> HashMap<String, String> {
        map! {
            "447153".to_string() => "bob".to_string(),
            "447160".to_string() => "bob".to_string(),
            "1864615".to_string() => "alice".to_string(),
            "2233095".to_string() => "charlie".to_string(),
            "2233469".to_string() => "charlie".to_string()
        }
    }

    // $ nvidia-smi pmon -c 1 -s mu
    #[test]
    fn test_extract_nvidia_pmon_processes() {
        let text = "# gpu        pid  type    sm   mem   enc   dec   command
# Idx          #   C/G     %     %     %     %   name
# gpu        pid  type    fb    sm   mem   enc   dec   command
# Idx          #   C/G    MB     %     %     %     %   name
    0     447153     C  7669     -     -     -     -   python3.9      
    0     447160     C 11057     -     -     -     -   python3.9      
    0     506826     C 11057     -     -     -     -   python3.9      
    0    1864615     C  1635    40     0     -     -   python         
    1    1864615     C   535     -     -     -     -   python         
    1    2233095     C 24395    84    23     -     -   python3        
    2    1864615     C   535     -     -     -     -   python         
    2    1448150     C  9383     -     -     -     -   python3        
    3    1864615     C   535     -     -     -     -   python         
    3    2233469     C 15771    90    23     -     -   python3        
";
        let processes = extract_nvidia_pmon_processes(text, &mkusers());
        assert!(
            processes
                == map! {
                    ("bob".to_string(), "447153".to_string(), "python3.9".to_string()) =>      (0b1, 0.0, 0.0, 7669*1024),
                    ("bob".to_string(), "447160".to_string(), "python3.9".to_string()) =>      (0b1, 0.0, 0.0, 11057*1024),
                    ("_zombie_506826".to_string(), "506826".to_string(), "python3.9".to_string()) => (0b1, 0.0, 0.0, 11057*1024),
                    ("alice".to_string(), "1864615".to_string(), "python".to_string()) =>      (0b1111, 40.0, 0.0, (1635+535+535+535)*1024),
                    ("charlie".to_string(), "2233095".to_string(), "python3".to_string()) =>   (0b10, 84.0, 23.0, 24395*1024),
                    ("_zombie_1448150".to_string(), "1448150".to_string(), "python3".to_string()) =>  (0b100, 0.0, 0.0, 9383*1024),
                    ("charlie".to_string(), "2233469".to_string(), "python3".to_string()) =>   (0b1000, 90.0, 23.0, 15771*1024)
                }
        );
    }

    // $ nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits
    #[test]
    fn test_extract_nvidia_query_processes() {
        let text = "2233095, 1190
3079002, 2350
1864615, 1426";
        let processes = extract_nvidia_query_processes(text, &mkusers());
        assert!(
            processes
                == map! {
                    ("_zombie_3079002".to_string(), "3079002".to_string(), "_unknown_".to_string()) => (!0, 0.0, 0.0, 2350*1024)
                }
        );
    }
}
