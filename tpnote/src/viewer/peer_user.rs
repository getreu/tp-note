//! Best-effort identification of the OS user owning the peer end of a
//! loopback TCP connection, used by the `viewer.same_user_policy` check.
//!
//! A loopback TCP connection carries no kernel-level peer-credential
//! primitive (`SO_PEERCRED` is Unix-domain-socket only; Windows has no TCP
//! equivalent), so we identify the peer indirectly: map the connection to its
//! owning process (`netstat2`), then that process to its user id (`sysinfo`),
//! and compare with our own. Both crates are cross-platform, so this is a
//! single code path on Linux, Windows and macOS.
//!
//! This is defense-in-depth, not a guarantee: the connection→PID→user lookup
//! is an enumerate-and-match with an inherent race, and the peer's user may be
//! indeterminate (sandbox / network namespace, platform privilege limits),
//! which is why the caller's policy decides what to do about `Unknown`.

use netstat2::{AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, get_sockets_info};
use std::net::SocketAddr;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, get_current_pid};

/// Outcome of comparing the connecting peer's OS user to our own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeerUser {
    /// The peer belongs to the same OS user as this process.
    Same,
    /// The peer belongs to a different OS user (proven).
    Other,
    /// The peer's OS user could not be determined (no matching socket, no
    /// PID, or no resolvable user id).
    Unknown,
}

/// Identify the OS user owning the peer end of a loopback TCP connection.
/// `local` is our accepted socket's `local_addr()`, `peer` its `peer_addr()`.
pub(crate) fn identify_peer_user(local: SocketAddr, peer: SocketAddr) -> PeerUser {
    // 1. connection -> owning PID of the peer (client) process.
    let Some(peer_pid) = peer_pid(local, peer) else {
        return PeerUser::Unknown;
    };
    // 2. Our own PID.
    let Ok(our_pid) = get_current_pid() else {
        return PeerUser::Unknown;
    };
    let peer_pid = Pid::from_u32(peer_pid);

    // 3. Resolve both PIDs to user ids. Refresh only the process(es) we care
    //    about. Two sysinfo 0.33 quirks to respect:
    //    - `everything()` is required: a narrower `ProcessRefreshKind`
    //      (e.g. `nothing().with_user(...)`) does not insert the process into
    //      the `System` map, so `process()` then returns `None`.
    //    - the PID list must be deduplicated: passing the same PID twice (which
    //      happens for a same-process/loopback self-connection) refreshes
    //      nothing. So collapse to one entry when peer and self coincide.
    let mut sys = System::new();
    let pids: Vec<Pid> = if peer_pid == our_pid {
        vec![peer_pid]
    } else {
        vec![peer_pid, our_pid]
    };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::everything(),
    );
    let peer_uid = sys.process(peer_pid).and_then(|p| p.user_id());
    let our_uid = sys.process(our_pid).and_then(|p| p.user_id());

    match (peer_uid, our_uid) {
        (Some(a), Some(b)) if a == b => PeerUser::Same,
        (Some(_), Some(_)) => PeerUser::Other,
        // Either user id was unresolvable.
        _ => PeerUser::Unknown,
    }
}

/// Find the PID owning the socket that is the peer end of our connection.
/// We match on **both** endpoints (the peer's `local` == our `peer`, and the
/// peer's `remote` == our `local`) so that port reuse cannot select the wrong
/// socket.
fn peer_pid(local: SocketAddr, peer: SocketAddr) -> Option<u32> {
    let sockets = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP,
    )
    .ok()?;
    for si in sockets {
        if let ProtocolSocketInfo::Tcp(tcp) = si.protocol_socket_info
            && tcp.local_addr == peer.ip()
            && tcp.local_port == peer.port()
            && tcp.remote_addr == local.ip()
            && tcp.remote_port == local.port()
        {
            return si.associated_pids.first().copied();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    /// A process connecting to itself is, by definition, the same OS user.
    /// This exercises the whole pipeline: `netstat2` enumeration, the
    /// both-endpoint match, and the `sysinfo` PID→uid comparison — the parts
    /// most likely to hide subtle bugs (address family, endpoint orientation).
    #[test]
    fn self_connection_is_same_user() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        // Keep the client socket alive for the duration of the lookup.
        let _client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        assert_eq!(
            identify_peer_user(server.local_addr().unwrap(), server.peer_addr().unwrap()),
            PeerUser::Same
        );
    }
}
