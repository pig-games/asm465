PLATFORMDEFS :?= false
.if !PLATFORMDEFS
PLATFORMDEFS := true

.comment

Based on https://github.com/lydon42/mandelbrot-explorer65/blob/main/include/mega65defs.s

.endcomment

dma .namespace
    COPY            = %00000000
    MIX             = %00000001
    SWAP            = %00000010
    FILL            = %00000011
    CHAIN           = %00000100   
    ADDRLSB_TRIG    = $D700
    ADDRMSB         = $D701
    ADDRBANK        = $D702
    CONTROL         = $D703 ; bit 1 = Enable F018b mode
    ADDRLSB_ETRIG   = $D705 ; LSB for MEGA65 DMA Extensions
    ETRIGINLINE     = $D707
    M65_SCREEN      = $0800
    M65_COLRAM      = $f800
.endnamespace ; dma

vic2 .namespace
    SPR0X     = $D000
    SPR0Y     = $D001
    SPR1X     = $D002
    SPR1Y     = $D003
    SPR2X     = $D004
    SPR2Y     = $D005
    SPR3X     = $D006
    SPR3Y     = $D007
    SPR4X     = $D008
    SPR4Y     = $D009
    SPR5X     = $D00A
    SPR5Y     = $D00B
    SPR6X     = $D00C
    SPR6Y     = $D00D
    SPR7X     = $D00E
    SPR7Y     = $D00F
    SPRXMSBS  = $D010
    SPTRENA   = $D015
.endnamespace ; vic2



.endif