// Code to talk to the sonar runner that runs as root and performs limited lookups of privileged
// information on our behalf.
//
// The protocol is defined in C code in ../runner/.

#[allow(dead_code)]
const REQ_EXIT: u8 = 0u8;
const REQ_EXE_FOR_PIDS: u8 = 1u8;

// Given a real PID, read the /proc/PID/exe link.
//
// (What we really want to do here is to send all the requests for all the PIDs at the same
// time, less overhead, but can optimize later.)

pub fn get_exe_path(pid: u32, request_fd: u32, response_fd: u32) -> String {
    let mut req = [0u8; 16];
    let mut ix = 0usize;
    ix = put_u32(&mut req, ix, 9u32); // Payload size
    ix = put_u8(&mut req, ix, REQ_EXE_FOR_PIDS);
    ix = put_u32(&mut req, ix, 1u32); // 1 pid
    ix = put_u32(&mut req, ix, pid); // The pid
    assert!(ix == 4 + 9);
    write(request_fd, &req, ix);
    let mut szbuf = [0u8; 4];
    read(response_fd, &mut szbuf, 4);
    let want;
    (want, _) = get_u32(&szbuf, 0);
    let mut rdbuf = Vec::<u8>::with_capacity(want as usize);
    rdbuf.resize(want as usize, 0u8);
    read(response_fd, rdbuf.as_mut_slice(), want as usize);
    let mut ix = 0;
    let op;
    (op, ix) = get_u8(rdbuf.as_slice(), ix);
    if op != REQ_EXE_FOR_PIDS {
        panic!("Bad op");
    }
    let npids;
    (npids, ix) = get_u32(&rdbuf[0..], ix);
    if npids != 1 {
        panic!("Bad num");
    }
    (_, ix) = get_u32(&rdbuf, ix);
    let slen;
    (slen, ix) = get_u32(&rdbuf, ix);
    get_string(&rdbuf[0..], ix, slen as usize).0
}

fn put_u8(req: &mut [u8], ix: usize, n: u8) -> usize {
    req[ix] = n;
    ix + 1
}

fn put_u32(req: &mut [u8], ix: usize, mut n: u32) -> usize {
    req[ix] = (n & 255) as u8;
    n >>= 8;
    req[ix + 1] = (n & 255) as u8;
    n >>= 8;
    req[ix + 2] = (n & 255) as u8;
    n >>= 8;
    req[ix + 3] = (n & 255) as u8;
    ix + 4
}

fn get_u8(buf: &[u8], ix: usize) -> (u8, usize) {
    (buf[ix], ix + 1)
}

fn get_u32(buf: &[u8], ix: usize) -> (u32, usize) {
    let mut n = buf[ix] as u32;
    n |= (buf[ix + 1] as u32) << 8;
    n |= (buf[ix + 2] as u32) << 8;
    n |= (buf[ix + 3] as u32) << 8;
    (n, ix + 4)
}

fn get_string(buf: &[u8], ix: usize, l: usize) -> (String, usize) {
    // FIXME: Needs to deal with UTF8.
    let mut s = "".to_string();
    for x in ix..ix + l {
        s.push(buf[x] as char);
    }
    (s, ix + l)
}

fn write(fd: u32, buf: &[u8], len: usize) {
    // FIXME: Must deal with partial writes
    let n = unsafe {
        libc::write(
            fd as i32,
            buf.as_ptr() as *const libc::c_void,
            len as libc::size_t,
        )
    };
    if (n as usize) < len {
        panic!("Wrong write");
    }
}

fn read(fd: u32, buf: &mut [u8], len: usize) {
    // FIXME: Must deal with partial reads
    let n = unsafe {
        libc::read(
            fd as i32,
            buf.as_ptr() as *mut libc::c_void,
            len as libc::size_t,
        )
    };
    if (n as usize) < len {
        panic!("Wrong read #1 {} {}", n, len);
    }
}
