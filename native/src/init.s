.cpu "4510"
.section init
    #enable40Mhz
    #enableVIC4Registers
    #disableC65ROM
    #disableCIAandIRQ

    ; set 80x50 character
	lda #80
	sta $D058
	lda #80
	sta $D05E
    lda #50
    sta $D07B

    ; set 640x400 mode
    lda $d031
    ora #%10001000
    sta $d031

    #setBasePage BasePage 

    sei 

    lda $d060
    sta ScreenPtr
    lda $d061
    sta ScreenPtr+1
    lda $d062
    sta ScreenPtr+2
    lda $d063
    sta ScreenPtr+3
    lda #0
    sta ColPtr
    sta ColPtr+1
    lda #$f8
    sta ColPtr+2
    lda #$0F
    sta ColPtr+3
.endsection
