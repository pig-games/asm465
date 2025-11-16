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
        SEL         = $DF30
        NUM         = $DF31
        ANIM        = $DF32
        XHI         = $DF33
        XLO         = $DF34
        YHI         = $DF35
        YLO         = $DF36
        SCL         = $DF37
    .endnamespace ; sprite
    system .namespace
        IRQ_PENDING  = $DF40  ; R   | Bitmask of pending IRQ sources (masked by enable when the line is asserted). |
        IRQ_ENABLE   = $DF41  ; R/W | Bitmask selecting which IRQ sources may raise the line. |
        IRQ_ACK      = $DF42  ; W   | Writing a bit clears the corresponding pending IRQ source. Reads return `IRQ_PENDING`. |
        IRQ_SOURCE   = $DF43  ; R   | Highest-priority pending IRQ source (lowest set bit), or `0xFF` if none. |
        NMI_PENDING  = $DF44  ; R   | Bitmask of pending NMI sources. |
        NMI_ACK      = $DF45  ; W   | Writing a bit clears the corresponding pending NMI source and drops the NMI line. Reads return `NMI_PENDING`. |
        CTRL_STATUS  = $DF46  ; R   | Debug/status bitfield (e.g., latched line state, overflow counters); reserved bits read as zero. 
    .endnamespace ; system
    gcontroller .namespace
        JOYBASE1         = $DF50   ; Joystick Port 1 directions + fire. Bits 0–4 active low: Up, Down, Left, Right, Fire.
        JOYBASE2         = $DF51   ; Joystick Port 2 directions + fire  bits 0–4 active low: Up, Down, Left, Right, Fire.
        JOYBASE3         = $DF52   ; Joystick Port 3 directions + fire. Bits 0–4 active low: Up, Down, Left, Right, Fire.
        JOYBASE4         = $DF53   ; Joystick Port 4 directions + fire  bits 0–4 active low: Up, Down, Left, Right, Fire.

    .endnamespace ; gcontroller
.endnamespace ; cross465


.endif
