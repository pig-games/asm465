PLATFORMDEFS :?= false
.if !PLATFORMDEFS
PLATFORMDEFS := true
; Cross465 platform definitions (C64-compatible, used like Mega65 defs)

; VIC-II
cross465 .namespace
    console .namespace
        PUTC        = $DF00  ;        0x00 => self.push_char(value),
        NL          = $DF01  ;        0x01 => self.newline(),
        PUTHEX      = $DF02  ;        0x02 => self.push_hex(value),
        CLR         = $DF03  ;        0x03 => self.clear(),
        SETX        = $DF04  ;        0x04 => self.set_x(value),
        SETY        = $DF05  ;        0x05 => self.set_y(value),
        SETLOC      = $DF06  ;        0x06 => self.set_location(),
        SETCOL      = $DF07  ;        0x07 => self.set_color(value),
        SETBGCOL    = $DF08  ;        0x08 => self.set_bg_color(value),
        SETLPTR     = $DF09  ;        0x09 => self.set_lptr(value),
        SETHPTR     = $DF0A  ;        0x0A => self.set_hptr(value),
        PRINT       = $DF0B  ;        0x0B => self.print(value),
    .endnamespace ; console
    display .namespace
        BRCOL       = $DF20
        BGCOL       = $DF21
    .endnamespace ; display
    sprite .namespace
        SPRSEL      = $DF30
        SPRNUM      = $DF31
        SPRANIM     = $DF32
        SPRXHI      = $DF33
        SPRXLO      = $DF34
        SPRYHI      = $DF35
        SPRYLO      = $DF36
        SPRSCL      = $DF37
    .endnamespace ; sprite
.endnamespace ; cross465


.endif
