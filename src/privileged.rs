// Code to talk to the sonar runner that runs as root and performs limited lookups of privileged
// information on our behalf.
//
// The protocol is defined in C code in ../runner/.
//
// Note that if the server is hung then these functions will hang too.  It may be that a watchdog
// timer is necessary to fix this: if the timer triggers, the server should be considered dead.  But
// this is probably too much.  For now, assume the server is reliable and will crash itself (and
// Sonar) if it gets into an error state.

#[allow(dead_code)]
const REQ_EXIT: u8 = 0u8;
const REQ_EXE_FOR_PIDS: u8 = 1u8;

// Given a real PID, read the /proc/PID/exe link.
//
// OPTIMIZEME: What we really want to do here is to send all the requests for all the PIDs at the
// same time, less overhead even if it means managing dynamic buffers.

pub fn get_exe_path(pid: u32, request_fd: u32, response_fd: u32) -> Result<String, String> {
    let mut req = [0u8; 16];
    let mut ix = 0usize;
    ix = put_u32(&mut req, ix, 9u32); // Payload size
    ix = put_u8(&mut req, ix, REQ_EXE_FOR_PIDS);
    ix = put_u32(&mut req, ix, 1u32); // 1 pid
    ix = put_u32(&mut req, ix, pid); // The pid
    assert!(ix == 4 + 9);
    write(request_fd, &req, ix)?;
    let mut szbuf = [0u8; 4];
    read(response_fd, &mut szbuf, 4)?;
    let want;
    (want, _) = get_u32(&szbuf, 0)?;
    let mut rdbuf = Vec::<u8>::with_capacity(want as usize);
    rdbuf.resize(want as usize, 0u8);
    read(response_fd, rdbuf.as_mut_slice(), want as usize)?;
    let mut ix = 0;
    let op;
    (op, ix) = get_u8(rdbuf.as_slice(), ix)?;
    if op != REQ_EXE_FOR_PIDS {
        return Err("Bad op".to_string());
    }
    let npids;
    (npids, ix) = get_u32(&rdbuf[0..], ix)?;
    if npids != 1 {
        return Err("Bad array length".to_string());
    }
    (_, ix) = get_u32(&rdbuf, ix)?;
    let slen;
    (slen, ix) = get_u32(&rdbuf, ix)?;
    Ok(get_string(&rdbuf[0..], ix, slen as usize)?.0)
}

// The putters will panic on OOB, this is OK.

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

fn get_u8(buf: &[u8], ix: usize) -> Result<(u8, usize), String> {
    if ix + 1 <= buf.len() {
        Ok((buf[ix], ix + 1))
    } else {
        Err("Out of bounds".to_string())
    }
}

fn get_u32(buf: &[u8], ix: usize) -> Result<(u32, usize), String> {
    if ix + 4 <= buf.len() {
        let mut n = buf[ix] as u32;
        n |= (buf[ix + 1] as u32) << 8;
        n |= (buf[ix + 2] as u32) << 8;
        n |= (buf[ix + 3] as u32) << 8;
        Ok((n, ix + 4))
    } else {
        Err("Out of bounds".to_string())
    }
}

fn get_string(buf: &[u8], ix: usize, l: usize) -> Result<(String, usize), String> {
    if ix + l <= buf.len() {
        Ok((
            String::from_utf8_lossy(&buf[ix..ix + l]).to_string(),
            ix + l,
        ))
    } else {
        Err("Out of bounds".to_string())
    }
}

// The reader and writer will panic on OOB, this is OK.

fn write(fd: u32, mut buf: &[u8], mut len: usize) -> Result<(), String> {
    while len > 0 {
        let n = unsafe {
            libc::write(
                fd as i32,
                buf.as_ptr() as *const libc::c_void,
                len as libc::size_t,
            )
        };
        if n == -1 {
            return Err("Write error".to_string());
        }
        let k = n as usize;
        buf = &buf[k..];
        len = len - k;
    }
    Ok(())
}

fn read(fd: u32, mut buf: &mut [u8], mut len: usize) -> Result<(), String> {
    while len > 0 {
        let n = unsafe {
            libc::read(
                fd as i32,
                buf.as_ptr() as *mut libc::c_void,
                len as libc::size_t,
            )
        };
        if n == -1 {
            return Err("Read error".to_string());
        }
        if n == 0 {
            break;
        }
        let k = n as usize;
        buf = &mut buf[k..];
        len = len - k;
    }
    Ok(())
}
