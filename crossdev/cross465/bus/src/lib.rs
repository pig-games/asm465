pub struct Memory {
    pub data: [u8; 0x10000],
}
impl Memory {
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        let mut a = start as usize;
        for &b in bytes {
            self.data[a & 0xFFFF] = b;
            a += 1;
        }
    }
    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }
    pub fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }
}
pub struct HostBridge {
    pub regs: [u8; 0x100],
}
impl HostBridge {
    pub fn new() -> Self {
        Self { regs: [0; 0x100] }
    }
}
pub struct Bus {
    ram: Memory,
    host: HostBridge,
}
impl Bus {
    pub fn new() -> Self {
        Self {
            ram: Memory::new(),
            host: HostBridge::new(),
        }
    }
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        self.ram.load(start, bytes);
    }
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xDF00..=0xDFFF => self.host.regs[(addr & 0xFF) as usize],
            _ => self.ram.read(addr),
        }
    }
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xDF00..=0xDFFF => self.host.regs[(addr & 0xFF) as usize] = val,
            _ => self.ram.write(addr, val),
        }
    }
    pub fn write_word(&mut self, addr: u16, word: u16) {
        self.write(addr, (word & 0xFF) as u8);
        self.write(addr + 1, (word >> 8) as u8);
    }
    #[cfg(any(test, feature = "testhooks"))]
    pub fn test_peek_host(&self, off: u8) -> u8 {
        self.host.regs[off as usize]
    }
    pub fn mem_mut(&mut self) -> &mut Memory {
        &mut self.ram
    }
    pub fn tick(&mut self, _cycles: u32) {
        // no-op for now; hook timers/MMIO here later
    }
}
