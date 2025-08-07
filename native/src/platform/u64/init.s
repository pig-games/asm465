.include "m65macros.h"

.section init
    cli
    #enable40Mhz
    #enableVIC4Registers
    #disableC65ROM
    #disableCIAandIRQ

    ; set 80x50 character
	lda #80
	sta vic4.LINESTEPLO
	lda #80
	sta vic4.CHRCOUNT
    lda #50
    sta vic4.DISPROWS

    ; set 640x400 mode
    lda vic3.SCRNMODE
    ora #%10001000
    sta vic3.SCRNMODE

    #setBasePage BasePage

    sei 

    lda vic4.SCRNPTR1
    sta ScreenPtr
    lda vic4.SCRNPTR2
    sta ScreenPtr+1
    lda vic4.SCRNPTR3
    sta ScreenPtr+2
    lda vic4.SCRNPTR4
    sta ScreenPtr+3
    lda #0
    sta ColPtr
    sta ColPtr+1
    lda #$f8
    sta ColPtr+2
    lda #$0F
    sta ColPtr+3
.endsection
