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

vic3 .namespace
    SM_H640  = $80
    SM_FAST  = $40
    SM_ATTR  = $20
    SM_BPM   = $10
    SM_V400  = $08
    SM_H1280 = $04
    SM_MONO  = $02
    SM_INT   = $01 
    ROMMAP   = $D030     ; ROME CROM9 ROMC ROMA ROM8 PAL   EXTSYNC CRAM2K
    SCRNMODE = $D031     ; H640 FAST  ATTR BPM  V400 H1280 MONO    INT
    PALRED   = $D100
    PALGRN   = $D200
    PALBLU   = $D300
.endnamespace ; vic3

vic4 .namespace
    SM_ALPHEN    = $80
    SM_VFAST     = $40
    SM_PALEMU    = $20
    SM_SPR640    = $10
    SM_SMTH      = $08
    SM_FCLRHI    = $04
    SM_FCLRLO    = $02
    SM_CHR16     = $01
  
    BORDERCOL    = $D020
    SCREENCOL    = $D021
    KEY          = $D02F
    SCRNMODE     = $D054     ; ALPHEN VFAST PALEMU SPR640 SMTH FCLRHI FCLRLO CHR16
    LINESTEPLO   = $D058
    LINESTEPHI   = $D059
    CHRCOUNT     = $D05E     ; how many characters to draw
    SCRNPTR1     = $D060
    SCRNPTR2     = $D061
    SCRNPTR3     = $D062
    SCRNPTR4     = $D063     ; EXGLYPH(1), EMPTY(1), CHRCOUNT(2), SCRNPTR(4)
    COLPTRLO     = $D064
    COLPTRHI     = $D065
    CHARPTRLO    = $D068
    CHARPTRHI    = $D069
    CHARPTRBN    = $D06A
    SPR16ENA     = $D06B
    SPRPTRADRLSB = $D06C
    SPRPTRADRMSB = $D06D
    SPRPTRADRBNK = $D06E
    PALETTE      = $D070     ; MAPEDPAL(2) BTPALSEL(2) SPRPALSEL(2) ABTPALSEL(2)
    DISPROWS     = $D07B
.endnamespace ; vic4

math .namespace
    BUSY     = $D70F
    IN_A1    = $D770
    IN_A2    = $D771
    IN_A3    = $D772
    IN_A4    = $D773
    IN_B1    = $D774
    IN_B2    = $D775
    IN_B3    = $D776
    IN_B4    = $D777
    MULTOUT1 = $D778
    MULTOUT2 = $D779
    MULTOUT3 = $D77A
    MULTOUT4 = $D77B
    MULTOUT5 = $D77C
    MULTOUT6 = $D77D
    MULTOUT7 = $D77E
    MULTOUT8 = $D77F
    DIVOUT1  = $D768
    DIVOUT2  = $D769
    DIVOUT3  = $D76A
    DIVOUT4  = $D76B
.endnamespace ; math