// -------------------------------------------------
// --------------------- 9P ------------------------
// -------------------------------------------------
// Implementation of the 9p filesystem device following the
// 9P2000.L protocol ( https://code.google.com/p/diod/wiki/protocol )

// TODO: forwarding search symlink inode is a little weird, the inode will be looked up in current inode list but it should be looked up in forwarder inode list
// TODO: do checks to see if INodeID and FileDescriptorID casted to each other to check for mistakes

use std::collections::hash_map::Entry;
use crate::filesystem::{FS};
use bitflags::bitflags;

use std::cmp::min;

//use crate::marshall::*;
//use crate::print_debug;
//use crate::wasi::wasi_print_internal;
use crate::wasi::*;
use crate::filesystem::*;
use std::collections::HashMap;
use std::ffi::CString;


pub type FileDescriptorID = usize;

// Feature bit (bit position) for mount tag.
// pub const VIRTIO_9P_F_MOUNT_TAG : i32 = 0;
// Assumed max tag length in bytes.
//pub const VIRTIO_9P_MAX_TAGLEN : i32 = 254;

//pub const MAX_REPLYBUFFER_SIZE : i32 = 16 * 1024 * 1024;

// TODO
// flush


#[allow(dead_code)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorNumber {
    SUCCESS = 0,
    /// Operation not permitted
    EPERM = 1,
    /// No such file or directory
    ENOENT = 2,
    /// Bad file descriptor
    EBADF = 8,
    /// File exists
    EEXIST = 17,
    /// Not a directory
    ENOTDIR = 20,
    /// Invalid argument
    EINVAL = 22,
    /// Directory not empty
    ENOTEMPTY = 39,
    /// The specified file descriptor refers to a pipe or FIFO.
    ESPIPE = 70,
    /// Protocol error
    EPROTO = 71,
    /// Not Capable
    ENOTCAPABLE = 76,
    /// Operation is not supported
    EOPNOTSUPP = 95
}



#[allow(dead_code)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum P9LockStatus {
    Success = 0,
    Blocked = 1,
    Error = 2,
    Grace = 3
}

//pub const FID_NONE : i32 = -1;
//pub const FID_INODE : i32 = 1;
//pub const FID_XATTR : i32 = 2;

pub const STDIN_FD : i32 = 0;
pub const STDOUT_FD : i32 = 1;
pub const STDERR_FD : i32 = 2;



#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FdFileType {
    /// The type of the file descriptor or file is unknown or is different from any of the other types specified.
    Unknown = 0, 
    /// The file descriptor or file refers to a block device inode.
    BlockDevice = 1,
    /// The file descriptor or file refers to a character device inode.
    CharacterDevice = 2,
    /// The file descriptor or file refers to a directory inode.
    Directory = 3,
    /// The file descriptor or file refers to a regular file inode.
    RegularFile = 4,
    /// The file descriptor or file refers to a datagram socket.
    SocketDGram = 5,
    /// The file descriptor or file refers to a byte-stream socket.
    SocketStream = 6,
    /// The file refers to a symbolic link inode.
    SymbolicLink = 7
}

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    pub struct FdFlags: u16 {
        /// Append mode: Data written to the file is always appended to the file's end.
        const Append = 1<<0;
        /// Write according to synchronized I/O data integrity completion. Only the data stored in the file is synchronized.
        const DSync = 1<<1;
        /// Non-blocking mode.
        const NonBlock = 1<<2;
        /// Synchronized read I/O operations.
        const RSync = 1<<3;
        /// Write according to synchronized I/O file integrity completion. In
        /// addition to synchronizing the data stored in the file, the implementation
        /// may also synchronously update the file's metadata.
        const Sync = 1<<4;
    }
}


bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    pub struct FdRights : u64 {
        /// The right to invoke `fd_datasync`.
        /// If `path_open` is set, includes the right to invoke
        /// `path_open` with `fdflags::dsync`.
        const FdDataSync = 1 << 0;
        /// The right to invoke `fd_read` and `sock_recv`.
        /// If `rights::fd_seek` is set, includes the right to invoke `fd_pread`.
        const FdRead = 1 << 1;
        /// The right to invoke `fd_seek`. This flag implies `rights::fd_tell`.
        const FdSeek = 1 << 2;
        /// The right to invoke `fd_fdstat_set_flags`.
        const FdFdstatSetFlags = 1 << 3;
        /// The right to invoke `fd_sync`.
        /// If `path_open` is set, includes the right to invoke
        /// `path_open` with `fdflags::rsync` and `fdflags::dsync`.
        const FdSync = 1 << 4;
        /// The right to invoke `fd_seek` in such a way that the file offset
        /// remains unaltered (i.e., `whence::cur` with offset zero), or to
        /// invoke `fd_tell`.
        const FdTell = 1 << 5;
        /// The right to invoke `fd_write` and `sock_send`.
        /// If `rights::fd_seek` is set, includes the right to invoke `fd_pwrite`.
        const FdWrite = 1 << 6;
        /// The right to invoke `fd_advise`.
        const FdAdvise = 1 << 7;
        /// The right to invoke `fd_allocate`.
        const FdAllocate = 1 << 8;
        /// The right to invoke `path_create_directory`.
        const PathCreateDirectory = 1 << 9;
        /// If `path_open` is set, the right to invoke `path_open` with `oflags::creat`.
        const PathCreateFile = 1 << 10;
        /// The right to invoke `path_link` with the file descriptor as the
        /// source directory.
        const PathLinkSource = 1 << 11;
        /// The right to invoke `path_link` with the file descriptor as the
        /// target directory.
        const PathLinkTarget = 1 << 12;
        /// The right to invoke `path_open`.
        const PathOpen = 1 << 13;
        /// The right to invoke `fd_readdir`.
        const FdReaddir = 1 << 14;
        /// The right to invoke `path_readlink`.
        const PathReadlink = 1 << 15;
        /// The right to invoke `path_rename` with the file descriptor as the source directory.
        const PathRenameSource = 1 << 16;
        /// The right to invoke `path_rename` with the file descriptor as the target directory.
        const PathRenameTarget = 1 << 17;
        /// The right to invoke `path_filestat_get`.
        const PathFiestatGet = 1 << 18;
        /// The right to change a file's size (there is no `path_filestat_set_size`).
        /// If `path_open` is set, includes the right to invoke `path_open` with `oflags::trunc`.
        const PathFilestatSetSize = 1 << 19;
        /// The right to invoke `path_filestat_set_times`.
        const PathFilestatSetTimes = 1 << 20;
        /// The right to invoke `fd_filestat_get`.
        const FdFilestatGet = 1 << 21;
        /// The right to invoke `fd_filestat_set_size`.
        const FdFilestatSetSize = 1 << 22;
        /// The right to invoke `fd_filestat_set_times`.
        const FdFilestatSetTimes = 1 << 23;
        /// The right to invoke `path_symlink`.
        const PathSymlink = 1 << 24;
        /// The right to invoke `path_remove_directory`.
        const PathRemoveDirectory = 1 << 25;
        /// The right to invoke `path_unlink_file`.
        const PathUnlinkFile = 1 << 26;
        /// If `rights::fd_read` is set, includes the right to invoke `poll_oneoff` to subscribe to `eventtype::fd_read`.
        /// If `rights::fd_write` is set, includes the right to invoke `poll_oneoff` to subscribe to `eventtype::fd_write`.
        const PollFdReadwrite = 1 << 27;
        /// The right to invoke `sock_shutdown`.
        const SockShutdown = 1 << 28;
        /// The right to invoke `sock_accept`.
        const SockAccept = 1 << 29;
    }
}

#[repr(C)]
pub struct FdStat {
    pub fs_filetype: FdFileType,
    pub fs_flags: FdFlags,
    pub fs_rights_base: FdRights,
    pub fs_rights_inheriting: FdRights,
}




pub type Device = u64;
pub type LinkCount = u64;
pub type FileSize = u64;
pub type Timestamp = u64;

pub const ROOT_DEVICE_ID : u64 = 0;

#[repr(C)]
pub struct FileStat {
    /// Device ID of device containing the file.
    pub dev: Device,
    /// File serial number.
    pub ino: INodeID,
    /// File type.
    pub filetype: FdFileType,
    /// Number of hard links to the file.
    pub nlink: LinkCount,
    /// For regular files, the file size in bytes. For symbolic links, the length in bytes of the pathname contained in the symbolic link.
    pub size: FileSize,
    /// Last data access timestamp.
    pub atim: Timestamp,
    /// Last data modification timestamp.
    pub mtim: Timestamp,
    /// Last file status change timestamp.
    pub ctim: Timestamp,
}



bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    pub struct FstFlags : u16 {
        /// Adjust the last data access timestamp to the value stored in `filestat::atim`.
        const Atim = 1 << 0;
        /// Adjust the last data access timestamp to the time of clock `clockid::realtime`.
        const AtimNow = 1 << 1;
        /// Adjust the last data modification timestamp to the value stored in `filestat::mtim`.
        const Mtim = 1 << 2;
        /// Adjust the last data modification timestamp to the time of clock `clockid::realtime`.
        const MtimNow = 1 << 3;
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Ciovec {
    /// The address of the buffer to be written.
    pub buf: *const u8,
    /// The length of the buffer to be written.
    pub buf_len: usize,
}

/*
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Iovec {
    /// The address of the buffer to be filled.
    pub buf: *mut u8,
    /// The length of the buffer to be filled.
    pub buf_len: usize,
}*/


#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DstBuf {
    pub buf: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SrcBuf {
    pub buf: *const u8,
    pub len: usize,
}
//pub type SrcIoVec<'a> = &'a [SrcBuf];
pub type DstIoVec<'a> = &'a [DstBuf];

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum PreStatDirectoryType {
    PreOpenTypeDir = 0
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PreStat {
    pub directory_type: PreStatDirectoryType,
    pub directory_path_len: usize
}

#[allow(dead_code)]
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum SeekWhence {
    /// The offset is set to the given value.
    Set = 0,
    /// The offset is set relative to the current position.
    Current = 1,
    /// The offset is set relative to the end of the file.
    End = 2,
}

#[allow(dead_code)]
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum SymlinkLookupFlags {
    /// If the path points to a symbolic link, the function will consider the symlink itself, not its target.
    NoFollow = 0,
    /// If the path points to a symbolic link, the function will follow the link and use the target of the symlink instead.
    Follow = 1
}

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[repr(C)]
    // docs from https://man7.org/linux/man-pages/man2/open.2.html
    pub struct FileOpenFlags : u32 {
        /////// these are access modes, must include one of them
        /// O_RDONLY, Open file for reading
        const O_RDONLY = 0;
        /// O_WRONLY, Open file for writing
        const O_WRONLY = 1;
        // O_RDWR, Open file for reading and writing
        const O_RDWR = 2;

        // file creation flags

        ///  Enable the close-on-exec flag for the new file descriptor.
        ///  Specifying this flag permits a program to avoid additional
        ///  fcntl F_SETFD operations to set the FD_CLOEXEC flag.
        const O_CLOEXEC = 02000000;
        ///   If pathname does not exist, create it as a regular file.
        const O_CREAT = 00000100;
        /// O_DIRECTORY
        /// If pathname is not a directory, cause the open to fail.
        const O_DIRECTORY = 00200000;
        /// O_EXCL
        ///  Ensure that this call creates the file: if this flag is
        ///  specified in conjunction with O_CREAT, and pathname
        ///  already exists, then open() fails with the error EEXIST.
        const O_EXCL = 00000200;
        
        /// O_NOFOLLOW
        ///  If the trailing component (i.e., basename) of pathname is
        ///  a symbolic link, then the open fails, with the error
        ///  ELOOP.  Symbolic links in earlier components of the
        ///  pathname will still be followed.  (Note that the ELOOP
        ///  error that can occur in this case is indistinguishable
        ///  from the case where an open fails because there are too
        ///  many symbolic links found while resolving components in
        ///  the prefix part of the pathname.)
        const O_NOFOLLOW = 00400000;

        /// O_TMPFILE
        ///  Create an unnamed temporary regular file.  The pathname
        ///  argument specifies a directory; an unnamed inode will be
        ///  created in that directory's filesystem.  Anything written
        ///  to the resulting file will be lost when the last file
        /// descriptor is closed, unless the file is given a name.
        const O_TMPFILE = 020000000 | 00200000;

        /// O_TRUNC
        /// If the file already exists and is a regular file and the
        /// access mode allows writing (i.e., is O_RDWR or O_WRONLY)
        /// it will be truncated to length 0.
        const O_TRUNC = 00001000;

        /// O_PATH
        /// Obtain a file descriptor that can be used for two
        ///       purposes: to indicate a location in the filesystem tree
        ///       and to perform operations that act purely at the file
        ///       descriptor level.  The file itself is not opened, and
        ///       other file operations (e.g., read(2), write(2), fchmod(2),
        ///       fchown(2), fgetxattr(2), ioctl(2), mmap(2)) fail with the
        ///       error EBADF.
        ///       The following operations can be performed on the resulting
        ///       file descriptor:
        ///       •  close(2).
        ///       •  fchdir(2), if the file descriptor refers to a directory
        ///          (since Linux 3.5).
        ///       •  fstat(2) (since Linux 3.6).
        ///       •  fstatfs(2) (since Linux 3.12).
        ///       •  Duplicating the file descriptor (dup(2), fcntl(2)
        ///          F_DUPFD, etc.).
        ///       •  Getting and setting file descriptor flags (fcntl(2)
        ///          F_GETFD and F_SETFD).
        ///       •  Retrieving open file status flags using the fcntl(2)
        ///          F_GETFL operation: the returned flags will include the
        ///          bit O_PATH.
        ///       •  Passing the file descriptor as the dirfd argument of
        ///          openat() and the other "*at()" system calls.  This
        ///          includes linkat(2) with AT_EMPTY_PATH (or via procfs
        ///          using AT_SYMLINK_FOLLOW) even if the file is not a
        ///          directory.
        ///       •  Passing the file descriptor to another process via a
        ///          UNIX domain socket (see SCM_RIGHTS in unix(7)).
        ///       When O_PATH is specified in flags, flag bits other than
        ///       O_CLOEXEC, O_DIRECTORY, and O_NOFOLLOW are ignored.
        const O_PATH = 010000000;

        //// file status flags
        
        /// O_APPEND  The file is opened in append mode.  Before each write(2),
        ///   the file offset is positioned at the end of the file, as
        ///   if with lseek.  The modification of the file offset and
        ///   the write operation are performed as a single atomic step.
        const O_APPEND = 00002000;
    }
}


#[repr(C)]
pub struct DirectoryEntry {
	d_ino : u64,
	d_off : i64,
	d_reclen : u16,
	d_type : u8,
	d_name : *mut u8,
}

// TODO: bus

pub struct Virtio9p {
    pub fs : FS,
    //pub bus : Bus
    //pub configspace_tagname : [i32; 6],
    //pub configspace_taglen : usize,
    //pub version : String,
    //pub blocksize : usize,
    //pub msize : usize,
    //pub replybuffer : UInt8Array,
    //pub replybuffersize : usize,
    pub file_descriptors : HashMap<FileDescriptorID, FileDescriptor>,
    pub next_fd : FileDescriptorID
}


const PIPE_MAX_FD : i32 = 2; 



pub struct FileDescriptor {
    pub inode_id : INodeID,
    pub flags : FdFlags,
    pub rights : FdRights,
    pub rights_inheriting : FdRights,
    pub fd : FileDescriptorID,
    pub offset : usize,
}

impl Virtio9p {
    /**
    * @constructor
    *
    * @param {FS} filesystem
    * @param {CPU} cpu
    */
    pub fn new(fs : Option<FS>) -> Virtio9p { // todo: pass in cpu and bus
        //let configspace_tagname = [0x68, 0x6F, 0x73, 0x74, 0x39, 0x70];
        //let msize = 8192;
        let fs_internal = 
            if fs.is_some() {
                fs.unwrap()
            } else {
                FS::new(None)
            };
        let result = Virtio9p {
            fs: fs_internal,
            //configspace_tagname: configspace_tagname, // "host9p" string
            //configspace_taglen: configspace_tagname.len(),
            //version: "9P2000.L".to_owned(),
            //blocksize: 8192,
            //msize: msize,
            //replybuffer: UInt8Array::new(msize*2),
            //replybuffersize: 0,
            file_descriptors : HashMap::new(),
            next_fd: 0 as FileDescriptorID,
        };

        return result;
    }

    pub fn get_pipe_fd(fd: i32) -> Option<Pipe> {
        match fd {
            STDIN_FD => Some(Pipe::Stdin),
            STDOUT_FD => Some(Pipe::Stdout),
            STDERR_FD => Some(Pipe::Stderr),
            _ => None,
        }
    }

    pub fn get_pipe_rights(pipe: Pipe) -> FdRights {
        match pipe {
            Pipe::Stdin => FdRights::FdRead 
                | FdRights::FdFilestatGet 
                | FdRights::PollFdReadwrite,
            Pipe::Stdout => FdRights::FdWrite 
                | FdRights::FdFilestatGet 
                | FdRights::PollFdReadwrite,
            Pipe::Stderr => FdRights::FdWrite 
                | FdRights::FdFilestatGet 
                | FdRights::PollFdReadwrite,
        }
    }

    pub fn get_fd(&self, fd: i32) -> Option<FileDescriptorID> {
        if fd > PIPE_MAX_FD && self.file_descriptors.contains_key(&(fd as FileDescriptorID)) {
            return Some(fd as FileDescriptorID)
        }
        else {
            None
        }
    }
    
    // Note: dbg_name is only used for debugging messages and may not be the same as the filename,
    // since it is not synchronised with renames done outside of 9p. Hard-links, linking and unlinking
    // operations also mean that having a single filename no longer makes sense.
    // Set TRACK_FILENAMES = true (in config.js) to sync dbg_name during 9p renames.
    pub fn create_fd(&mut self, inode_id: INodeID, flags : FdFlags, rights: FdRights, rights_inheriting: FdRights) -> &mut FileDescriptor {
        let file_descriptor = FileDescriptor {
            inode_id: inode_id, 
            flags : flags,
            rights: rights,
            rights_inheriting: rights_inheriting,
            offset: 0,
            fd: self.next_fd
        };
        self.next_fd += 1;
        let fd = file_descriptor.fd;
        match self.file_descriptors.entry(fd) {
            Entry::Occupied(e) => &mut *e.into_mut(),
            Entry::Vacant(e) => {
                &mut *e.insert(file_descriptor)
            }
        }
    }

    pub fn close_fd(&mut self, fd: FileDescriptorID) -> ErrorNumber {
        if let Some(mut file_descriptor) = self.file_descriptors.remove(&fd) {
            // remove all rights
            file_descriptor.rights = FdRights::empty();
            file_descriptor.rights_inheriting = FdRights::empty();
            ErrorNumber::SUCCESS
        }
        else {
            ErrorNumber::ENOENT
        }
    }

    pub fn allocate(&mut self, fd: FileDescriptorID, offset: i64, len: i64) -> ErrorNumber {
        let inode_id = self.file_descriptors[&fd].inode_id;
        if self.fs.get_size(inode_id) < (offset + len) as usize {
            self.fs.change_size(inode_id, (offset+len) as usize);
        }
        ErrorNumber::SUCCESS
    }

    fn get_inode_filetype(&self, inode_id: INodeID) -> FdFileType {
        if self.fs.is_directory(inode_id) {
            FdFileType::Directory
        } else if self.fs.get_inode(inode_id).mode == S_IFLNK {
            FdFileType::SymbolicLink
        } else {
            FdFileType::RegularFile
        }
    }

    pub fn fd_stat(&self, fd: FileDescriptorID, stat: &mut FdStat) -> ErrorNumber {
        let file_descriptor = &self.file_descriptors[&fd];
        stat.fs_filetype = self.get_inode_filetype(file_descriptor.inode_id);
        stat.fs_flags = file_descriptor.flags;
        stat.fs_rights_base = file_descriptor.rights;
        stat.fs_rights_inheriting = file_descriptor.rights_inheriting;
        ErrorNumber::SUCCESS
    }

    pub fn fd_stat_set_flags(&mut self, fd: FileDescriptorID, flags: FdFlags) -> ErrorNumber {
        let file_descriptor = self.file_descriptors.get_mut(&fd).unwrap();
        file_descriptor.flags = flags;
        ErrorNumber::SUCCESS
    }

    pub fn fd_stat_set_rights(&mut self, fd: FileDescriptorID, rights: FdRights, rights_inheriting: FdRights) -> ErrorNumber {
        let file_descriptor = self.file_descriptors.get_mut(&fd).unwrap();
        // only allowed to remove capabilities, not add them
        if (rights | file_descriptor.rights) != file_descriptor.rights ||
           (rights_inheriting | file_descriptor.rights_inheriting) != rights_inheriting {
            ErrorNumber::ENOTCAPABLE
        }
        else {
            file_descriptor.rights = rights;
            file_descriptor.rights_inheriting = rights_inheriting;
            ErrorNumber::SUCCESS
        }
    }

    pub fn get_file_stat(&self, fd: FileDescriptorID, filestat : &mut FileStat) -> ErrorNumber{
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = file_descriptor.inode_id;
        self.get_file_stat_from_inode_id(inode_id, filestat)
    }
    pub fn get_file_stat_from_inode_id(&self, inode_id: INodeID, filestat : &mut FileStat) -> ErrorNumber {
        let inode = &self.fs.get_inode(inode_id);
        // Device ID of device containing the file.
        filestat.dev = if let Some(dev_id) = inode.mount_id {
                dev_id as u64
            } else {
                ROOT_DEVICE_ID
            };
        // File serial number.
        filestat.ino = inode_id;
        // File type.
        filestat.filetype = self.get_inode_filetype(inode_id);
        // Number of hard links to the file.
        filestat.nlink = inode.nlinks as u64;
        // For regular files, the file size in bytes. For symbolic links, the length in bytes of the pathname contained in the symbolic link.
        filestat.size = if filestat.filetype == FdFileType::SymbolicLink {
                inode.symlink.as_bytes().len() as u64
            } else {
                inode.size as u64
            };
        // Last data access timestamp.
        filestat.atim = inode.atime;
        // Last data modification timestamp.
        filestat.mtim = inode.mtime;
        // Last file status change timestamp.
        filestat.ctim = inode.ctime;
        ErrorNumber::SUCCESS
    }

    pub fn file_stat_set_size(&mut self, fd : FileDescriptorID, size : usize) -> ErrorNumber {
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = file_descriptor.inode_id;
        // only change size of files, not symlinks or directories
        if self.get_inode_filetype(inode_id) != FdFileType::RegularFile {
            return ErrorNumber::EOPNOTSUPP;
        }

        self.fs.change_size(inode_id, size);

        ErrorNumber::SUCCESS
    }

    pub fn file_stat_set_times(&mut self, fd : FileDescriptorID, atim: Timestamp, mtim: Timestamp, fst_flags: FstFlags) -> ErrorNumber {
        self.file_stat_set_times_from_inode_id(
            self.file_descriptors[&fd].inode_id,
            atim,
            mtim,
            fst_flags)
    }

    pub fn file_stat_set_times_from_inode_id(&mut self, inode_id : INodeID, atim: Timestamp, mtim: Timestamp, fst_flags: FstFlags) -> ErrorNumber {
        let inode = &mut self.fs.get_inode_mutable(inode_id);
        
        // Adjust the last data access timestamp to the value stored in `filestat::atim`.
        if (fst_flags & FstFlags::Atim) == FstFlags::Atim {
            inode.atime = atim;
        }

        // Adjust the last data access timestamp to the time of clock `clockid::realtime`.
        if (fst_flags & FstFlags::AtimNow) == FstFlags::AtimNow {
            inode.atime = FS::seconds_since_epoch();
        }

        // Adjust the last data modification timestamp to the value stored in `filestat::mtim`.
        if (fst_flags & FstFlags::Mtim) == FstFlags::Mtim {
            inode.mtime = mtim;
        }

        // Adjust the last data modification timestamp to the time of clock `clockid::realtime`.
        if (fst_flags & FstFlags::AtimNow) == FstFlags::AtimNow {
            inode.mtime = FS::seconds_since_epoch();
        }
        
        ErrorNumber::SUCCESS
    }

    // note: the order can vary due to hashmap so cookie not a great idea
    pub fn read_dir(&mut self,
        dir_fd : FileDescriptorID,
        dir_entries: &mut [DirectoryEntry],
        cookie : usize,
        bufused : &mut usize) -> ErrorNumber
    {
        let inode_id = self.file_descriptors[&dir_fd].inode_id;
        let directories = self.fs.read_dir_from_inode(inode_id);
        let entries_writing = min(directories.len(), dir_entries.len());
        for i in cookie..entries_writing {
            let c_string = CString::new(&*directories[i]).expect("CString::new failed");
            // Get the raw pointer and prevent the CString from being dropped
            dir_entries[i].d_reclen = (19 + c_string.as_bytes().len()) as u16;
            // Assign the pointer to d_name
            let raw_ptr = c_string.into_raw();
            dir_entries[i].d_name = raw_ptr as *mut u8;
            dir_entries[i].d_off = i as i64;
            // 19 from other entries
        }
        *bufused = entries_writing;
        ErrorNumber::SUCCESS
    }

    pub fn read_vec(&mut self, fd: FileDescriptorID, dst: DstIoVec, offset: Option<usize>, n_read: &mut usize) -> ErrorNumber {
        *n_read = 0;
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = self.file_descriptors[&fd].inode_id;

        // use specified offset, otherwise file descriptor offset
        let mut cur_offset = 
            if let Some(offset_val) = offset {
                offset_val
            } else {
                file_descriptor.offset
            };

        for buf in dst {
            let n_reading = buf.len;
            let buf_writing_to = unsafe { std::slice::from_raw_parts_mut(buf.buf, n_reading) };
            if let Some(data) = self.fs.read(inode_id, cur_offset, n_reading) {
                buf_writing_to.copy_from_slice(data);
            }
            else {
                return ErrorNumber::EINVAL; // failed to read, maybe out of bounds
            }
            cur_offset += n_reading;
            *n_read += n_reading;
        }

        // if not specified offset, update file descriptor's offset
        if offset.is_none() {
            self.file_descriptors.get_mut(&fd).unwrap().offset = cur_offset;
        }

        ErrorNumber::SUCCESS
    }

    pub fn write_vec(&mut self, fd: FileDescriptorID, src: &[SrcBuf], offset: Option<usize>, n_written: &mut usize) -> ErrorNumber {
        *n_written = 0;
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = file_descriptor.inode_id;
        // if don't specify offset, use offset in file_descriptor
        let mut cur_offset = 
            if let Some(offset_val) = offset {
                offset_val
            } else {
                file_descriptor.offset
            };

        for buf in src {
            let n_writing = buf.len;
            let buf_reading_from = unsafe { std::slice::from_raw_parts(buf.buf, n_writing) };
            self.fs.write_arr(inode_id, cur_offset, n_writing, Some(buf_reading_from));
            cur_offset += n_writing;
            *n_written += n_writing;
        }
        // if didn't specify offset, update file descriptor offset
        if offset.is_none() {
            self.file_descriptors.get_mut(&fd).unwrap().offset = cur_offset;
        }
        ErrorNumber::SUCCESS
    }

    pub fn prestat_get(&mut self, fd: FileDescriptorID, prestat : &mut PreStat) -> ErrorNumber{
        let inode_id = self.file_descriptors[&fd].inode_id;

        if self.get_inode_filetype(inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // only valid for directories
        }
        prestat.directory_type = PreStatDirectoryType::PreOpenTypeDir; // only one valid type right now
        prestat.directory_path_len = self.fs.get_full_path(inode_id).as_bytes().len();

        ErrorNumber::SUCCESS
    }

    pub fn prestat_dir_name(&mut self, fd: FileDescriptorID, buffer: &mut [u8]) -> ErrorNumber {
        let inode_id = self.file_descriptors[&fd].inode_id;

        if self.get_inode_filetype(inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // only valid for directories
        }

        let path = self.fs.get_full_path(inode_id);
        let path_bytes = path.as_bytes();

        // copy directory name into buffer
        buffer.copy_from_slice(&path_bytes[0..min(path_bytes.len(), buffer.len())]);

        ErrorNumber::SUCCESS
    }

    pub fn tell(&self, fd : FileDescriptorID, offset : &mut usize) -> ErrorNumber {
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = file_descriptor.inode_id;

        if self.get_inode_filetype(inode_id) != FdFileType::RegularFile {
            return ErrorNumber::EINVAL; // only valid for files
        }
        *offset = file_descriptor.offset;
        ErrorNumber::SUCCESS
    }

    pub fn seek(&mut self, fd: FileDescriptorID, offset: i64, whence: SeekWhence, newoffset: &mut usize) -> ErrorNumber {
        let file_descriptor = &self.file_descriptors[&fd];
        let inode_id = file_descriptor.inode_id;
        let inode = self.fs.get_inode(inode_id);

        if self.get_inode_filetype(inode_id) != FdFileType::RegularFile {
            return ErrorNumber::EINVAL; // only valid for files
        }

        // we need to use i64 to allow for negative numbers
        let new_offset : i64 = match whence {
            // The offset is set to the given value.
            SeekWhence::Set => offset,
            // The offset is set relative to the current position.
            SeekWhence::Current => (file_descriptor.offset as i64) + offset,
            // The offset is set relative to the end of the file.
            SeekWhence::End => (inode.size as i64) - offset - 1 // -1 because for example, a file of size 1 and offset 0 should be at 0 
        };

        // out of bounds check
        if new_offset < 0 || new_offset >= inode.size as i64 {
            ErrorNumber::EINVAL
        }
        else {
            // it's alright, update it
            self.file_descriptors.get_mut(&fd).unwrap().offset = new_offset as usize;
            *newoffset = new_offset as usize;
            ErrorNumber::SUCCESS
        }
    }

    pub fn create_directory(&mut self, parent_fd: FileDescriptorID, name: &str) -> ErrorNumber {
        let parent_inode_id = self.file_descriptors[&parent_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // only valid for parents that are directories
        }
        let _result_inode_id = self.fs.create_directory(name, Some(parent_inode_id));
        return ErrorNumber::SUCCESS
    }

    // This isn't technically correct when we have multiple mounted file systems because inode could be on a seperate mounted file system
    // however, WASI doesn't support multiple mounted file systems, so it's ok for now
    pub fn lookup_path_inode(&mut self, parent_fd: FileDescriptorID, symlink_flags: SymlinkLookupFlags, path: &str) -> Option<INodeID> {
        let parent_inode_id = self.file_descriptors[&parent_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            None
        } else if let Some(searched_inode_id) = self.fs.search(parent_inode_id, path) {
            if self.fs.is_symlink(searched_inode_id) {
                match symlink_flags {
                    SymlinkLookupFlags::NoFollow => Some(searched_inode_id),
                    SymlinkLookupFlags::Follow => self.fs.follow_symlink(parent_inode_id, searched_inode_id)
                }
            } else {
                Some(searched_inode_id)
            }
        }
        else {
            None
        }
    }

    pub fn path_file_stat_get(&mut self, parent_fd: FileDescriptorID, symlink_flags: SymlinkLookupFlags, path: &str, stat: &mut FileStat) -> ErrorNumber {
        if let Some(inode_id) = self.lookup_path_inode(parent_fd, symlink_flags, path) {
            self.get_file_stat_from_inode_id(inode_id, stat)
        }
        else {
            ErrorNumber::EBADF
        }
    }

    pub fn path_file_stat_set_times(&mut self,
        parent_fd: FileDescriptorID,
        symlink_flags: SymlinkLookupFlags,
        path: &str,
        atim: Timestamp,
        mtim: Timestamp,
        fst_flags: FstFlags) -> ErrorNumber
    {   
        if let Some(inode_id) = self.lookup_path_inode(parent_fd, symlink_flags, path) {
            self.file_stat_set_times_from_inode_id(
                inode_id,
                atim,
                mtim,
                fst_flags)
        }
        else {
            ErrorNumber::EBADF
        }
    }

    pub fn link(&mut self, 
        old_parent_dir_fd : FileDescriptorID,
        old_symlink_flags : SymlinkLookupFlags,
        old_path_str : &str,
        new_parent_fd : FileDescriptorID,
        new_path_str : &str) -> ErrorNumber {
        if let Some(old_inode_id) = self.lookup_path_inode(old_parent_dir_fd, old_symlink_flags, old_path_str) {
            let new_parent_inode_id = self.file_descriptors[&new_parent_fd].inode_id;
            if self.get_inode_filetype(new_parent_inode_id) != FdFileType::Directory {
                ErrorNumber::ENOTDIR
            } else {
                // link attaches the old inode to under the new parent as well, using new_path_str
                self.fs.link(new_parent_inode_id, old_inode_id, new_path_str)
            }
        }
        else {
            ErrorNumber::EBADF
        }
    }

    pub fn path_open(&mut self,
        parent_dir_fd : FileDescriptorID,
        parent_symlink_flags : SymlinkLookupFlags,
        path_str : &str,
        oflags : FileOpenFlags, 
        fs_rights_base : FdRights,
        fs_rights_inheriting : FdRights,
        fdflags : FdFlags,
        fd_out_ref : &mut FileDescriptorID) -> ErrorNumber
    {
        // Todo: FileOpenFlags::O_NOFOLLOW
        // Todo: FileOpenFlags::O_PATH (it works, is just too permissive)
        // Todo: FileOpenFlags::TMP_FILE (we don't yet support those)
        let parent_inode_id = self.file_descriptors[&parent_dir_fd].inode_id;
        
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR;
        }

        let opened_inode_id = 
            if let Some(inode_id) = self.lookup_path_inode(parent_dir_fd, parent_symlink_flags, path_str) {
                // O_EXCL and O_CREAT means that we throw this error if file exists
                if oflags.contains(FileOpenFlags::O_EXCL)
                    && oflags.contains(FileOpenFlags::O_CREAT) {
                    return ErrorNumber::EEXIST;
                }
                // O_DIRECTORY means it must be a directory
                if oflags.contains(FileOpenFlags::O_DIRECTORY)
                    && self.get_inode_filetype(inode_id) != FdFileType::Directory {
                    return ErrorNumber::ENOTDIR;
                }
                // O_TMPFILE specifies that inode_id actually refers to a directory that we put our file in
                if oflags.contains(FileOpenFlags::O_TMPFILE) {
                    // TODO: tmpfile
                    return ErrorNumber::EOPNOTSUPP;
                }
                // Otherwise, open the file as specified
                inode_id  
            }        
            else if oflags.contains(FileOpenFlags::O_CREAT) {
                if oflags.contains(FileOpenFlags::O_DIRECTORY) {
                    self.fs.create_directory(path_str, Some(parent_inode_id))
                } else {
                    self.fs.create_file(path_str, parent_inode_id)
                }
            } else {
                return ErrorNumber::EBADF;
            };

        // precompute for borrow checker
        let inode_size = self.fs.get_inode(opened_inode_id).size;

        let file_descriptor = self.create_fd(
            opened_inode_id,
            fdflags,
            fs_rights_base, 
            fs_rights_inheriting);
        let file_descriptor_fd = file_descriptor.fd;
        // truncate file if truncate flag and writing/readwrite
        if oflags.contains(FileOpenFlags::O_TRUNC)
            && (oflags.contains(FileOpenFlags::O_WRONLY) ||
                oflags.contains(FileOpenFlags::O_RDWR)) {
            self.file_stat_set_size(file_descriptor_fd, 0);
        }
        // append means that we start at the end (it also means we always go to the end upon write,
        // but that happens anyway, so this is sufficient)
        // we have an else because if we've truncated, no need to set offset to zero
        // the else also satisfies the borrow checker
        else if oflags.contains(FileOpenFlags::O_APPEND) {
            file_descriptor.offset = inode_size;
        }

        *fd_out_ref = file_descriptor_fd;
        ErrorNumber::SUCCESS
    }

    pub fn path_read_link(&mut self, parent_dir_fd: FileDescriptorID, path_str: &str, out_buf: &mut [u8], out_buf_used: &mut usize) -> ErrorNumber {
        let parent_inode_id = self.file_descriptors[&parent_dir_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // not in a directory
        }

        if let Some(inode_id) = self.fs.search(parent_inode_id, path_str) {
            if !self.fs.is_symlink(inode_id) {
                return ErrorNumber::EINVAL; // not a symbolic link
            }
            let symlink_bytes = self.fs.get_inode(inode_id).symlink.as_bytes();
            let bytes_reading = min(symlink_bytes.len(), out_buf.len());
            // we should truncate to however many bytes are given, according to docs https://linux.die.net/man/2/readlink
            out_buf.copy_from_slice(&symlink_bytes[..bytes_reading]);
            *out_buf_used = bytes_reading;
            ErrorNumber::SUCCESS
        }
        else {
            ErrorNumber::EBADF // does not exist
        }
    }

    pub fn path_unlink_dir(&mut self, parent_dir_fd: FileDescriptorID, path_str: &str) -> ErrorNumber {
        let parent_inode_id = self.file_descriptors[&parent_dir_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // not in a directory
        }

        if let Some(inode_id) = self.fs.search(parent_inode_id, path_str) {
            if self.get_inode_filetype(inode_id) != FdFileType::Directory {
                return ErrorNumber::ENOTDIR; // not a directory
            }
            // TODO: This should consider forwarders, but that doesn't matter for wasm bc no mounting
            self.fs.unlink_from_dir(parent_inode_id, path_str);
            ErrorNumber::SUCCESS
        }
        else {
            ErrorNumber::EBADF // does not exist
        }
    }
    pub fn path_unlink_file(&mut self, parent_dir_fd: FileDescriptorID, path_str: &str) -> ErrorNumber {
        let parent_inode_id = self.file_descriptors[&parent_dir_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::EINVAL; // not in a directory
        }

        if let Some(inode_id) = self.fs.search(parent_inode_id, path_str) {
            if self.get_inode_filetype(inode_id) != FdFileType::RegularFile {
                return ErrorNumber::ENOTDIR; // not a file
            }
            self.fs.unlink(parent_inode_id, path_str)
        }
        else {
            ErrorNumber::EBADF // does not exist
        }
    }

    pub fn rename(&mut self, old_parent_dir_fd : FileDescriptorID,
        old_path_str : &str,
        new_parent_dir_fd : FileDescriptorID,
        new_path_str : &str) -> ErrorNumber
    {
        let old_parent_inode_id = self.file_descriptors[&old_parent_dir_fd].inode_id;
        if self.get_inode_filetype(old_parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // not in a directory
        }
        let new_parent_inode_id = self.file_descriptors[&new_parent_dir_fd].inode_id;
        if self.get_inode_filetype(new_parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // not in a directory
        }

        self.fs.rename(old_parent_inode_id,
            old_path_str,
            new_parent_inode_id,
            new_path_str)
    }

    pub fn symlink(&mut self, parent_fd: FileDescriptorID, old_path_str: &str, new_path_str: &str) -> ErrorNumber {
        let parent_inode_id = self.file_descriptors[&parent_fd].inode_id;
        if self.get_inode_filetype(parent_inode_id) != FdFileType::Directory {
            return ErrorNumber::ENOTDIR; // not in a directory
        }

        let _symlink_node_id = self.fs.create_symlink(old_path_str, parent_inode_id, new_path_str);
        ErrorNumber::SUCCESS
    }


    pub fn renumber(&mut self, from_fd : FileDescriptorID, to_fd : FileDescriptorID) -> ErrorNumber {
        let from_inode_id = self.file_descriptors[&from_fd].inode_id;
        self.file_descriptors.get_mut(&to_fd).unwrap().inode_id = from_inode_id;
        // rights and flags are preserved and not modified
        return ErrorNumber::SUCCESS;
    }



    /*
        /** @type {VirtIO} */
        this.virtio = new VirtIO(cpu,
        {
            name: "virtio-9p",
            pci_id: 0x06 << 3,
            device_id: 0x1049,
            subsystem_device_id: 9,
            common:
            {
                initial_port: 0xA800,
                queues:
                [
                    {
                        size_supported: 32,
                        notify_offset: 0,
                    },
                ],
                features:
                [
                    VIRTIO_9P_F_MOUNT_TAG,
                    VIRTIO_F_VERSION_1,
                    VIRTIO_F_RING_EVENT_IDX,
                    VIRTIO_F_RING_INDIRECT_DESC,
                ],
                on_driver_ok: () => {},
            },
            notification:
            {
                initial_port: 0xA900,
                single_handler: false,
                handlers:
                [
                    (queue_id) =>
                    {
                        if(queue_id !== 0)
                        {
                            dbg_assert(false, "Virtio9P Notified for non-existent queue: " + queue_id +
                                " (expected queue_id of 0)");
                            return;
                        }
                        while(this.virtqueue.has_request())
                        {
                            const bufchain = this.virtqueue.pop_request();
                            this.ReceiveRequest(bufchain);
                        }
                        this.virtqueue.notify_me_after(0);
                        // Don't flush replies here: async replies are not completed yet.
                    },
                ],
            },
            isr_status:
            {
                initial_port: 0xA700,
            },
            device_specific:
            {
                initial_port: 0xA600,
                struct:
                [
                    {
                        bytes: 2,
                        name: "mount tag length",
                        read: () => this.configspace_taglen,
                        write: data => { /* read only */ },
                    },
                ].concat(v86util.range(VIRTIO_9P_MAX_TAGLEN).map(index =>
                    ({
                        bytes: 1,
                        name: "mount tag name " + index,
                        // Note: configspace_tagname may have changed after set_state
                        read: () => this.configspace_tagname[index] || 0,
                        write: data => { /* read only */ },
                    })
                )),
            },
        })
        this.virtqueue = this.virtio.queues[0];    


    pub fn get_state ()
    {
        var state = [];

        state[0] = this.configspace_tagname;
        state[1] = this.configspace_taglen;
        state[2] = this.virtio;
        state[3] = this.VERSION;
        state[4] = this. LOCKSIZE;
        state[5] = this.msize;
        state[6] = this.replybuffer;
        state[7] = this.replybuffersize;
        state[8] = this.fids.map(function(f) { return [f.inodeid, f.type, f.uid, f.dbg_name]; });
        state[9] = this.fs;

        return state;
    }

    pub fn set_state(state)
    {
        this.configspace_tagname = state[0];
        this.configspace_taglen = state[1];
        this.virtio.set_state(state[2]);
        this.virtqueue = this.virtio.queues[0];
        this.VERSION = state[3];
        this.BLOCKSIZE = state[4];
        this.msize = state[5];
        this.replybuffer = state[6];
        this.replybuffersize = state[7];
        this.fids = state[8].map(function(f)
        {
            return { inodeid: f[0], type: f[1], uid: f[2], dbg_name: f[3] };
        });
        this.fs.set_state(state[9]);
    }
    */

    /*
    pub fn update_dbg_name(&mut self, idx: usize, newname: &str)
    {
        for fid in &mut self.fids
        {
            if fid.inodeid == idx {
                fid.dbg_name = newname.to_owned().clone();
            } 
        }
    }

    pub fn reset(&mut self) {
        self.fids = HashMap::new();
        //TODO self.virtio.reset();    
    }

    pub fn build_reply(&mut self, id : u8, tag : u16, payloadsize : usize) {
        if (payloadsize+7) >= self.replybuffer.data.len() {
            print_debug!("Error in 9p: payloadsize exceeds maximum length");
        }
        let data = &mut *self.replybuffer.data;
        let mut offset = 0;
        offset = marshall_u32((payloadsize+7) as u32, data, offset);
        offset = marshall_u8(id+1, data, offset);
        marshall_u16(tag, data, offset);
        //for(var i=0; i<payload.length; i++)
        //    this.replybuffer[7+i] = payload[i];
        self.replybuffersize = payloadsize+7;
    }

    pub fn send_error(&mut self, tag: u16, errormsg: String, errorcode: u32) {
        //var size = marshall.Marshall(["s", "w"], [errormsg, errorcode], this.replybuffer, 7);
        let data = &mut *self.replybuffer.data;
        let size = marshall_u32(errorcode, data, 7); // put error code after the stuff written below
        self.build_reply(6, tag, size as usize);
    }
    */
    /*
    pub fn send_reply(bufchain) {
        dbg_assert(this.replybuffersize >= 0, "9P: Negative replybuffersize");
        bufchain.set_next_blob(this.replybuffer.subarray(0, this.replybuffersize));
        this.virtqueue.push_reply(bufchain);
        this.virtqueue.flush_replies();
    }
    pub fn receive_request(bufchain) {
        // TODO: split into header + data blobs to avoid unnecessary copying.
        const buffer = new Uint8Array(bufchain.length_readable);
        bufchain.get_next_blob(buffer);

        const state = { offset : 0 };
        var header = marshall.Unmarshall(["w", "b", "h"], buffer, state);
        var size = header[0];
        var id = header[1];
        var tag = header[2];
        //message.Debug("size:" + size + " id:" + id + " tag:" + tag);

        switch(id)
        {
            case 8: // statfs
                size = this.fs.GetTotalSize(); // size used by all files
                var space = this.fs.GetSpace();
                var req = [];
                req[0] = 0x01021997;
                req[1] = this.BLOCKSIZE; // optimal transfer block size
                req[2] = Math.floor(space/req[1]); // free blocks
                req[3] = req[2] - Math.floor(size/req[1]); // free blocks in fs
                req[4] = req[2] - Math.floor(size/req[1]); // free blocks avail to non-superuser
                req[5] = this.fs.CountUsedInodes(); // total number of inodes
                req[6] = this.fs.CountFreeInodes();
                req[7] = 0; // file system id?
                req[8] = 256; // maximum length of filenames

                size = marshall.Marshall(["w", "w", "d", "d", "d", "d", "d", "d", "w"], req, this.replybuffer, 7);
                this.BuildReply(id, tag, size);
                this.SendReply(bufchain);
                break;

            case 112: // topen
            case 12: // tlopen
                var req = marshall.Unmarshall(["w", "w"], buffer, state);
                var fid = req[0];
                var mode = req[1];
                message.Debug("[open] fid=" + fid + ", mode=" + mode);
                var idx = this.fids[fid].inodeid;
                var inode = this.fs.GetInode(idx);
                message.Debug("file open " + this.fids[fid].dbg_name);
                //if (inode.status === STATUS_LOADING) return;
                var ret = this.fs.OpenInode(idx, mode);

                this.fs.AddEvent(this.fids[fid].inodeid,
                    function() {
                        message.Debug("file opened " + this.fids[fid].dbg_name + " tag:"+tag);
                        var req = [];
                        req[0] = inode.qid;
                        req[1] = this.msize - 24;
                        marshall.Marshall(["Q", "w"], req, this.replybuffer, 7);
                        this.BuildReply(id, tag, 13+4);
                        this.SendReply(bufchain);
                    }.bind(this)
                );
                break;

            case 70: // link
                var req = marshall.Unmarshall(["w", "w", "s"], buffer, state);
                var dfid = req[0];
                var fid = req[1];
                var name = req[2];
                message.Debug("[link] dfid=" + dfid + ", name=" + name);

                var ret = this.fs.Link(this.fids[dfid].inodeid, this.fids[fid].inodeid, name);

                if(ret < 0)
                {
                    let error_message = "";
                    if(ret === -EPERM) error_message = "Operation not permitted";
                    else
                    {
                        error_message = "Unknown error: " + (-ret);
                        dbg_assert(false, "[link]: Unexpected error code: " + (-ret));
                    }
                    this.SendError(tag, error_message, -ret);
                    this.SendReply(bufchain);
                    break;
                }

                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 16: // symlink
                var req = marshall.Unmarshall(["w", "s", "s", "w"], buffer, state);
                var fid = req[0];
                var name = req[1];
                var symgt = req[2];
                var gid = req[3];
                message.Debug("[symlink] fid=" + fid + ", name=" + name + ", symgt=" + symgt + ", gid=" + gid);
                var idx = this.fs.CreateSymlink(name, this.fids[fid].inodeid, symgt);
                var inode = this.fs.GetInode(idx);
                inode.uid = this.fids[fid].uid;
                inode.gid = gid;
                marshall.Marshall(["Q"], [inode.qid], this.replybuffer, 7);
                this.BuildReply(id, tag, 13);
                this.SendReply(bufchain);
                break;

            case 18: // mknod
                var req = marshall.Unmarshall(["w", "s", "w", "w", "w", "w"], buffer, state);
                var fid = req[0];
                var name = req[1];
                var mode = req[2];
                var major = req[3];
                var minor = req[4];
                var gid = req[5];
                message.Debug("[mknod] fid=" + fid + ", name=" + name + ", major=" + major + ", minor=" + minor+ "");
                var idx = this.fs.CreateNode(name, this.fids[fid].inodeid, major, minor);
                var inode = this.fs.GetInode(idx);
                inode.mode = mode;
                //inode.mode = mode | S_IFCHR; // XXX: fails "Mknod - fifo" test
                inode.uid = this.fids[fid].uid;
                inode.gid = gid;
                marshall.Marshall(["Q"], [inode.qid], this.replybuffer, 7);
                this.BuildReply(id, tag, 13);
                this.SendReply(bufchain);
                break;


            case 22: // TREADLINK
                var req = marshall.Unmarshall(["w"], buffer, state);
                var fid = req[0];
                var inode = this.fs.GetInode(this.fids[fid].inodeid);
                message.Debug("[readlink] fid=" + fid + " name=" + this.fids[fid].dbg_name + " target=" + inode.symlink);
                size = marshall.Marshall(["s"], [inode.symlink], this.replybuffer, 7);
                this.BuildReply(id, tag, size);
                this.SendReply(bufchain);
                break;


            case 72: // tmkdir
                var req = marshall.Unmarshall(["w", "s", "w", "w"], buffer, state);
                var fid = req[0];
                var name = req[1];
                var mode = req[2];
                var gid = req[3];
                message.Debug("[mkdir] fid=" + fid + ", name=" + name + ", mode=" + mode + ", gid=" + gid);
                var idx = this.fs.CreateDirectory(name, this.fids[fid].inodeid);
                var inode = this.fs.GetInode(idx);
                inode.mode = mode | S_IFDIR;
                inode.uid = this.fids[fid].uid;
                inode.gid = gid;
                marshall.Marshall(["Q"], [inode.qid], this.replybuffer, 7);
                this.BuildReply(id, tag, 13);
                this.SendReply(bufchain);
                break;

            case 14: // tlcreate
                var req = marshall.Unmarshall(["w", "s", "w", "w", "w"], buffer, state);
                var fid = req[0];
                var name = req[1];
                var flags = req[2];
                var mode = req[3];
                var gid = req[4];
                this.bus.send("9p-create", [name, this.fids[fid].inodeid]);
                message.Debug("[create] fid=" + fid + ", name=" + name + ", flags=" + flags + ", mode=" + mode + ", gid=" + gid);
                var idx = this.fs.CreateFile(name, this.fids[fid].inodeid);
                this.fids[fid].inodeid = idx;
                this.fids[fid].type = FID_INODE;
                this.fids[fid].dbg_name = name;
                var inode = this.fs.GetInode(idx);
                inode.uid = this.fids[fid].uid;
                inode.gid = gid;
                inode.mode = mode | S_IFREG;
                marshall.Marshall(["Q", "w"], [inode.qid, this.msize - 24], this.replybuffer, 7);
                this.BuildReply(id, tag, 13+4);
                this.SendReply(bufchain);
                break;

            case 52: // lock
                var req = marshall.Unmarshall(["w", "b", "w", "d", "d", "w", "s"], buffer, state);
                var fid = req[0];
                var flags = req[2];
                var lock_length = req[4] === 0 ? Infinity : req[4];
                var lock_request = this.fs.DescribeLock(req[1], req[3], lock_length, req[5], req[6]);
                message.Debug("[lock] fid=" + fid +
                    ", type=" + P9_LOCK_TYPES[lock_request.type] + ", start=" + lock_request.start +
                    ", length=" + lock_request.length + ", proc_id=" + lock_request.proc_id);

                var ret = this.fs.Lock(this.fids[fid].inodeid, lock_request, flags);

                marshall.Marshall(["b"], [ret], this.replybuffer, 7);
                this.BuildReply(id, tag, 1);
                this.SendReply(bufchain);
                break;

            case 54: // getlock
                var req = marshall.Unmarshall(["w", "b", "d", "d", "w", "s"], buffer, state);
                var fid = req[0];
                var lock_length = req[3] === 0 ? Infinity : req[3];
                var lock_request = this.fs.DescribeLock(req[1], req[2], lock_length, req[4], req[5]);
                message.Debug("[getlock] fid=" + fid +
                    ", type=" + P9_LOCK_TYPES[lock_request.type] + ", start=" + lock_request.start +
                    ", length=" + lock_request.length + ", proc_id=" + lock_request.proc_id);

                var ret = this.fs.GetLock(this.fids[fid].inodeid, lock_request);

                if(!ret)
                {
                    ret = lock_request;
                    ret.type = P9_LOCK_TYPE_UNLCK;
                }

                var ret_length = ret.length === Infinity ? 0 : ret.length;

                size = marshall.Marshall(["b", "d", "d", "w", "s"],
                    [ret.type, ret.start, ret_length, ret.proc_id, ret.client_id],
                    this.replybuffer, 7);

                this.BuildReply(id, tag, size);
                this.SendReply(bufchain);
                break;

            case 24: // getattr
                var req = marshall.Unmarshall(["w", "d"], buffer, state);
                var fid = req[0];
                var inode = this.fs.GetInode(this.fids[fid].inodeid);
                message.Debug("[getattr]: fid=" + fid + " name=" + this.fids[fid].dbg_name + " request mask=" + req[1]);
                if(!inode || inode.status === STATUS_UNLINKED)
                {
                    message.Debug("getattr: unlinked");
                    this.SendError(tag, "No such file or directory", ENOENT);
                    this.SendReply(bufchain);
                    break;
                }
                req[0] = req[1]; // request mask
                req[1] = inode.qid;

                req[2] = inode.mode;
                req[3] = inode.uid; // user id
                req[4] = inode.gid; // group id

                req[5] = inode.nlinks; // number of hard links
                req[6] = (inode.major<<8) | (inode.minor); // device id low
                req[7] = inode.size; // size low
                req[8] = this.BLOCKSIZE;
                req[9] = Math.floor(inode.size/512+1); // blk size low
                req[10] = inode.atime; // atime
                req[11] = 0x0;
                req[12] = inode.mtime; // mtime
                req[13] = 0x0;
                req[14] = inode.ctime; // ctime
                req[15] = 0x0;
                req[16] = 0x0; // btime
                req[17] = 0x0;
                req[18] = 0x0; // st_gen
                req[19] = 0x0; // data_version
                marshall.Marshall([
                "d", "Q",
                "w",
                "w", "w",
                "d", "d",
                "d", "d", "d",
                "d", "d", // atime
                "d", "d", // mtime
                "d", "d", // ctime
                "d", "d", // btime
                "d", "d",
                ], req, this.replybuffer, 7);
                this.BuildReply(id, tag, 8 + 13 + 4 + 4+ 4 + 8*15);
                this.SendReply(bufchain);
                break;

            case 26: // setattr
                var req = marshall.Unmarshall(["w", "w",
                    "w", // mode
                    "w", "w", // uid, gid
                    "d", // size
                    "d", "d", // atime
                    "d", "d", // mtime
                ], buffer, state);
                var fid = req[0];
                var inode = this.fs.GetInode(this.fids[fid].inodeid);
                message.Debug("[setattr]: fid=" + fid + " request mask=" + req[1] + " name=" + this.fids[fid].dbg_name);
                if(req[1] & P9_SETATTR_MODE) {
                    // XXX: check mode (S_IFREG or S_IFDIR or similar should be set)
                    inode.mode = req[2];
                }
                if(req[1] & P9_SETATTR_UID) {
                    inode.uid = req[3];
                }
                if(req[1] & P9_SETATTR_GID) {
                    inode.gid = req[4];
                }
                if(req[1] & P9_SETATTR_ATIME) {
                    inode.atime = Math.floor((new Date()).getTime()/1000);
                }
                if(req[1] & P9_SETATTR_MTIME) {
                    inode.mtime = Math.floor((new Date()).getTime()/1000);
                }
                if(req[1] & P9_SETATTR_CTIME) {
                    inode.ctime = Math.floor((new Date()).getTime()/1000);
                }
                if(req[1] & P9_SETATTR_ATIME_SET) {
                    inode.atime = req[6];
                }
                if(req[1] & P9_SETATTR_MTIME_SET) {
                    inode.mtime = req[8];
                }
                if(req[1] & P9_SETATTR_SIZE) {
                    await this.fs.ChangeSize(this.fids[fid].inodeid, req[5]);
                }
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 50: // fsync
                var req = marshall.Unmarshall(["w", "d"], buffer, state);
                var fid = req[0];
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 40: // TREADDIR
            case 116: // read
                var req = marshall.Unmarshall(["w", "d", "w"], buffer, state);
                var fid = req[0];
                var offset = req[1];
                var count = req[2];
                var inode = this.fs.GetInode(this.fids[fid].inodeid);
                if(id === 40) message.Debug("[treaddir]: fid=" + fid + " offset=" + offset + " count=" + count);
                if(id === 116) message.Debug("[read]: fid=" + fid + " (" + this.fids[fid].dbg_name + ") offset=" + offset + " count=" + count + " fidtype=" + this.fids[fid].type);
                if(!inode || inode.status === STATUS_UNLINKED)
                {
                    message.Debug("read/treaddir: unlinked");
                    this.SendError(tag, "No such file or directory", ENOENT);
                    this.SendReply(bufchain);
                    break;
                }
                if(this.fids[fid].type === FID_XATTR) {
                    if(inode.caps.length < offset+count) count = inode.caps.length - offset;
                    for(var i=0; i<count; i++)
                        this.replybuffer[7+4+i] = inode.caps[offset+i];
                    marshall.Marshall(["w"], [count], this.replybuffer, 7);
                    this.BuildReply(id, tag, 4 + count);
                    this.SendReply(bufchain);
                } else {
                    this.fs.OpenInode(this.fids[fid].inodeid, undefined);
                    const inodeid = this.fids[fid].inodeid;

                    count = Math.min(count, this.replybuffer.length - (7 + 4));

                    if(inode.size < offset+count) count = inode.size - offset;
                    else if(id === 40)
                    {
                        // for directories, return whole number of dir-entries.
                        count = this.fs.RoundToDirentry(inodeid, offset + count) - offset;
                    }
                    if(offset > inode.size)
                    {
                        // offset can be greater than available - should return count of zero.
                        // See http://ericvh.github.io/9p-rfc/rfc9p2000.html#anchor30
                        count = 0;
                    }

                    this.bus.send("9p-read-start", [this.fids[fid].dbg_name]);

                    const data = await this.fs.Read(inodeid, offset, count);

                    this.bus.send("9p-read-end", [this.fids[fid].dbg_name, count]);

                    if(data) {
                        this.replybuffer.set(data, 7 + 4);
                    }
                    marshall.Marshall(["w"], [count], this.replybuffer, 7);
                    this.BuildReply(id, tag, 4 + count);
                    this.SendReply(bufchain);
                }
                break;

            case 118: // write
                var req = marshall.Unmarshall(["w", "d", "w"], buffer, state);
                var fid = req[0];
                var offset = req[1];
                var count = req[2];

                const filename = this.fids[fid].dbg_name;

                message.Debug("[write]: fid=" + fid + " (" + filename + ") offset=" + offset + " count=" + count + " fidtype=" + this.fids[fid].type);
                if(this.fids[fid].type === FID_XATTR)
                {
                    // XXX: xattr not supported yet. Ignore write.
                    this.SendError(tag, "Setxattr not supported", EOPNOTSUPP);
                    this.SendReply(bufchain);
                    break;
                }
                else
                {
                    // XXX: Size of the subarray is unchecked
                    await this.fs.Write(this.fids[fid].inodeid, offset, count, buffer.subarray(state.offset));
                }

                this.bus.send("9p-write-end", [filename, count]);

                marshall.Marshall(["w"], [count], this.replybuffer, 7);
                this.BuildReply(id, tag, 4);
                this.SendReply(bufchain);
                break;

            case 74: // RENAMEAT
                var req = marshall.Unmarshall(["w", "s", "w", "s"], buffer, state);
                var olddirfid = req[0];
                var oldname = req[1];
                var newdirfid = req[2];
                var newname = req[3];
                message.Debug("[renameat]: oldname=" + oldname + " newname=" + newname);
                var ret = await this.fs.Rename(this.fids[olddirfid].inodeid, oldname, this.fids[newdirfid].inodeid, newname);
                if(ret < 0) {
                    let error_message = "";
                    if(ret === -ENOENT) error_message = "No such file or directory";
                    else if(ret === -EPERM) error_message = "Operation not permitted";
                    else if(ret === -ENOTEMPTY) error_message = "Directory not empty";
                    else
                    {
                        error_message = "Unknown error: " + (-ret);
                        dbg_assert(false, "[renameat]: Unexpected error code: " + (-ret));
                    }
                    this.SendError(tag, error_message, -ret);
                    this.SendReply(bufchain);
                    break;
                }
                if(TRACK_FILENAMES)
                {
                    const newidx = this.fs.Search(this.fids[newdirfid].inodeid, newname);
                    this.update_dbg_name(newidx, newname);
                }
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 76: // TUNLINKAT
                var req = marshall.Unmarshall(["w", "s", "w"], buffer, state);
                var dirfd = req[0];
                var name = req[1];
                var flags = req[2];
                message.Debug("[unlink]: dirfd=" + dirfd + " name=" + name + " flags=" + flags);
                var fid = this.fs.Search(this.fids[dirfd].inodeid, name);
                if(fid === -1) {
                    this.SendError(tag, "No such file or directory", ENOENT);
                    this.SendReply(bufchain);
                    break;
                }
                var ret = this.fs.Unlink(this.fids[dirfd].inodeid, name);
                if(ret < 0) {
                    let error_message = "";
                    if(ret === -ENOTEMPTY) error_message = "Directory not empty";
                    else if(ret === -EPERM) error_message = "Operation not permitted";
                    else
                    {
                        error_message = "Unknown error: " + (-ret);
                        dbg_assert(false, "[unlink]: Unexpected error code: " + (-ret));
                    }
                    this.SendError(tag, error_message, -ret);
                    this.SendReply(bufchain);
                    break;
                }
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 100: // version
                var version = marshall.Unmarshall(["w", "s"], buffer, state);
                message.Debug("[version]: msize=" + version[0] + " version=" + version[1]);
                if(this.msize !== version[0])
                {
                    this.msize = version[0];
                    this.replybuffer = new Uint8Array(Math.min(MAX_REPLYBUFFER_SIZE, this.msize*2));
                }
                size = marshall.Marshall(["w", "s"], [this.msize, this.VERSION], this.replybuffer, 7);
                this.BuildReply(id, tag, size);
                this.SendReply(bufchain);
                break;

            case 104: // attach
                // return root directorie's QID
                var req = marshall.Unmarshall(["w", "w", "s", "s", "w"], buffer, state);
                var fid = req[0];
                var uid = req[4];
                message.Debug("[attach]: fid=" + fid + " afid=" + hex8(req[1]) + " uname=" + req[2] + " aname=" + req[3]);
                this.fids[fid] = this.Createfid(0, FID_INODE, uid, "");
                var inode = this.fs.GetInode(this.fids[fid].inodeid);
                marshall.Marshall(["Q"], [inode.qid], this.replybuffer, 7);
                this.BuildReply(id, tag, 13);
                this.SendReply(bufchain);
                this.bus.send("9p-attach");
                break;

            case 108: // tflush
                var req = marshall.Unmarshall(["h"], buffer, state);
                var oldtag = req[0];
                message.Debug("[flush] " + tag);
                //marshall.Marshall(["Q"], [inode.qid], this.replybuffer, 7);
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;


            case 110: // walk
                var req = marshall.Unmarshall(["w", "w", "h"], buffer, state);
                var fid = req[0];
                var nwfid = req[1];
                var nwname = req[2];
                message.Debug("[walk]: fid=" + req[0] + " nwfid=" + req[1] + " nwname=" + nwname);
                if(nwname === 0) {
                    this.fids[nwfid] = this.Createfid(this.fids[fid].inodeid, FID_INODE, this.fids[fid].uid, this.fids[fid].dbg_name);
                    //this.fids[nwfid].inodeid = this.fids[fid].inodeid;
                    marshall.Marshall(["h"], [0], this.replybuffer, 7);
                    this.BuildReply(id, tag, 2);
                    this.SendReply(bufchain);
                    break;
                }
                var wnames = [];
                for(var i=0; i<nwname; i++) {
                    wnames.push("s");
                }
                var walk = marshall.Unmarshall(wnames, buffer, state);
                var idx = this.fids[fid].inodeid;
                var offset = 7+2;
                var nwidx = 0;
                //console.log(idx, this.fs.GetInode(idx));
                message.Debug("walk in dir " + this.fids[fid].dbg_name  + " to: " + walk.toString());
                for(var i=0; i<nwname; i++) {
                    idx = this.fs.Search(idx, walk[i]);

                    if(idx === -1) {
                    message.Debug("Could not find: " + walk[i]);
                    break;
                    }
                    offset += marshall.Marshall(["Q"], [this.fs.GetInode(idx).qid], this.replybuffer, offset);
                    nwidx++;
                    //message.Debug(this.fids[nwfid].inodeid);
                    //this.fids[nwfid].inodeid = idx;
                    //this.fids[nwfid].type = FID_INODE;
                    this.fids[nwfid] = this.Createfid(idx, FID_INODE, this.fids[fid].uid, walk[i]);
                }
                marshall.Marshall(["h"], [nwidx], this.replybuffer, 7);
                this.BuildReply(id, tag, offset-7);
                this.SendReply(bufchain);
                break;

            case 120: // clunk
                var req = marshall.Unmarshall(["w"], buffer, state);
                message.Debug("[clunk]: fid=" + req[0]);
                if(this.fids[req[0]] && this.fids[req[0]].inodeid >=  0) {
                    await this.fs.CloseInode(this.fids[req[0]].inodeid);
                    this.fids[req[0]].inodeid = -1;
                    this.fids[req[0]].type = FID_NONE;
                }
                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                break;

            case 32: // txattrcreate
                var req = marshall.Unmarshall(["w", "s", "d", "w"], buffer, state);
                var fid = req[0];
                var name = req[1];
                var attr_size = req[2];
                var flags = req[3];
                message.Debug("[txattrcreate]: fid=" + fid + " name=" + name + " attr_size=" + attr_size + " flags=" + flags);

                // XXX: xattr not supported yet. E.g. checks corresponding to the flags needed.
                this.fids[fid].type = FID_XATTR;

                this.BuildReply(id, tag, 0);
                this.SendReply(bufchain);
                //this.SendError(tag, "Operation i not supported",  EINVAL);
                //this.SendReply(bufchain);
                break;

            case 30: // xattrwalk
                var req = marshall.Unmarshall(["w", "w", "s"], buffer, state);
                var fid = req[0];
                var newfid = req[1];
                var name = req[2];
                message.Debug("[xattrwalk]: fid=" + req[0] + " newfid=" + req[1] + " name=" + req[2]);

                // Workaround for Linux restarts writes until full blocksize
                this.SendError(tag, "Setxattr not supported", EOPNOTSUPP);
                this.SendReply(bufchain);
                /*
                this.fids[newfid] = this.Createfid(this.fids[fid].inodeid, FID_NONE, this.fids[fid].uid, this.fids[fid].dbg_name);
                //this.fids[newfid].inodeid = this.fids[fid].inodeid;
                //this.fids[newfid].type = FID_NONE;
                var length = 0;
                if (name === "security.capability") {
                    length = this.fs.PrepareCAPs(this.fids[fid].inodeid);
                    this.fids[newfid].type = FID_XATTR;
                }
                marshall.Marshall(["d"], [length], this.replybuffer, 7);
                this.BuildReply(id, tag, 8);
                this.SendReply(bufchain);
                */
                break;

            default:
                message.Debug("Error in Virtio9p: Unknown id " + id + " received");
                message.Abort();
                //this.SendError(tag, "Operation i not supported",  EOPNOTSUPP);
                //this.SendReply(bufchain);
                break;
        }

        //consistency checks if there are problems with the filesystem
        //this.fs.Check();
    };

    }




    }
    */
}


