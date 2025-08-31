PLATFORMDEFS :?= false
.if !PLATFORMDEFS
PLATFORMDEFS := true
; Cross465 platform definitions (C64-compatible, used like Mega65 defs)

; VIC-II
cross465 .namespace
    PUTC = $DF00  ;        0x00 => self.push_char(value),
    NL = $DF01  ;        0x01 => self.newline(),
    PUTHEX = $DF02  ;        0x02 => self.push_hex(value),
    CLR = $DF03  ;        0x03 => self.clear(),
    SETX = $DF04  ;        0x04 => self.set_x(value),
    SETY = $DF05  ;        0x05 => self.set_y(value),
    SETLOC = $DF06  ;        0x06 => self.set_location(),
    SETCOL = $DF07  ;        0x07 => self.set_color(value),
    SETBGCOL = $DF08  ;        0x08 => self.set_bg_color(value),
.endnamespace ; cross465


.endif