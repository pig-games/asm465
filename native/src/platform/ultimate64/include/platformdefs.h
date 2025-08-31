PLATFORMDEFS :?= false
.if !PLATFORMDEFS
PLATFORMDEFS := true
; Ultimate64 platform definitions (C64-compatible, used like Mega65 defs)

; CIA
CIA1_BASE  = $DC00
CIA2_BASE  = $DD00

; VIC-II
vic2 .namespace
    VIC_BASE   = $D000
    BORDERCOL  = $D020
    SCREENCOL  = $D021
    BGCOLOR0   = $D021
    RASTER     = $D012
    RASTERHI   = $D011
    CHARSET    = $D018
.endnamespace ; vic2

; KERNAL
CHROUT     = $FFD2
SCINIT     = $FF81
CLRCHN     = $FFCC
IOINIT     = $FF84
HOME       = $E3A6
CLRSCN     = $E544

; Screen memory / color RAM
SCRN_BASE  = $0400
COLR_BASE  = $D800
SCRN_W     = 40
SCRN_H     = 25

.endif