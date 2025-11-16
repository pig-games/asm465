     ; Based on the great work by Shallan at: https://github.com/smnjameson
PLATFORMMACROS :?= false
.if !PLATFORMMACROS
PLATFORMMACROS := true
.include "platformdefs.h"

phq .function
	pha
	phx
	phy
	phz
.endfunction

plq .function
	plz
	ply
	plx
	pla
.endfunction

BasicUpstart .macro addr
		.byte $09,$20 ; End of command marker (first byte after the 00 terminator)
		.byte $0A,$00 ; 10
		.byte $fe,$02,$30,$00 ; BANK 0
		.byte <end, >end  ; End of command marker (first byte after the 00 terminator)
		.byte $14,$00 ; 20
		.byte $9e  ; SYS
		.text format("%4d", \addr)
		.byte $00
	end
		.byte $00,$00	;End of basic terminators
.endmacro

enable40Mhz .macro
		lda #$41
		sta $00 ; 40 Mhz mode
.endmacro

enableVIC3Registers .macro
		lda #$00
		tax 
		tay 
		taz 
		map
		eom

		lda #$A5	;Enable VIC III
		sta vic4.KEY
		lda #$96
		sta vic4.KEY
.endmacro

enableVIC4Registers .macro
		lda #$00
		tax 
		tay 
		taz 
		map
		eom

		lda #$47	;Enable VIC IV
		sta vic4.KEY
		lda #$53
		sta vic4.KEY
		eom
.endmacro

disableCIAandIRQ .macro
    	lda #$7f
        sta $DC0D 
        sta $DD0D 

        lda #$00
        sta $D01A

        lda #$70
        sta $D640
        nop
.endmacro

disableC65ROM .macro
		lda #$70
		sta $d640
		eom
.endmacro

setBasePage .macro addr
        lda #>\addr
        tab
.endmacro

mapMemory .macro source, target
	sourceMB 	 .var (\source & $ff00000) >> 20
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffHi	 .var \sourceOffset >> 16
	sourceOffLo  .var (\sourceOffset & $0ff00 ) >> 8
	bitLo 		 .var pow(2, (((\target) & $ff00) >> 12) / 2) << 4
	bitHi 	 	 .var pow(2, (((\target-$8000) & $ff00) >> 12) / 2) << 4
	
	.if \target<$8000
		lda #sourceMB
		ldx #$0f
		ldy #$00
		ldz #$00
	.else
		lda #$00
		ldx #$00
		ldy #sourceMB
		ldz #$0f
	.endif
	map 

	; Set offset map
	.if \target<$8000
		lda #sourceOffLo
		ldx #(sourceOffHi + bitLo)
		ldy #$00
		ldz #$00
	.else
		lda #$00
		ldx #$00
		ldy #sourceOffLo
		ldz #(sourceOffHi + bitHi)
	.endif	
	map 
	eom
.endmacro

.namespace vic4
SetCharLocation .macro addr
	lda #(\addr & $ff)
	sta $d068
	lda #((\addr & $ff00)>>8)
	sta $d069
	lda #((\addr & $ff0000)>>16)
	sta $d06a
.endmacro

SetScreenLocation .macro addr
	lda #(\addr & $ff)
	sta $d060
	lda #(\addr & $ff00)>>8)
	sta $d061
	lda #((\addr & $ff0000)>>16)
	sta $d062
	lda #(((\addr & $ff0000)>>24) & $0f)
	sta $d063
.endmacro
.endnamespace ; vic4

.namespace dma
runJob .macro jobPointer
		lda #(\jobPointer >> 16)
		sta $d702
		sta $d704
		lda #>\jobPointer
		sta $d701
		lda #<\jobPointer
		sta $d705
.endmacro

header .macro sourceBank, destBank
		.byte $0A ; Request format is F018A
		.byte $80, \sourceBank
		.byte $81, \destBank
.endmacro

step .macro sourceStep, sourceStepFractional, destStep, destStepFractional
		.if \sourceStepFractional != 0
			.byte $82, \sourceStepFractional
		.endif
		.if \sourceStep != 1
			.byte $83, \sourceStep
		.endif
		.if \destStepFractional != 0
			.byte $84, \destStepFractional
		.endif
		.if \destStep != 1
			.byte $85, \destStep
		.endif
.endmacro

disableTransparency .macro
		.byte $06
.endmacro

enableTransparency .macro transparentByte
		.byte $07 
		.byte $86, \transparentByte
.endmacro

copyJob .macro source, destination, length, chain, backwards
	.byte $00 ; No more options
	.if \chain
		.byte $04 ; Copy and chain
	.else
		.byte $00 ; Copy and last request
	.endif
	
	backByte .var 0
	.if \backwards
		.eval backByte = $40
		.eval \source = \source + \length - 1
		.eval \destination = \destination + \length - 1
	.endif
	.word \length ; Size of Copy

	.word \source & $ff
	.byte (\source >> 16) + backByte

	.word \destination & $ffff
	.byte ((\destination >> 16) & $0f) + backByte
	.if \chain
		.word $0000
	.endif
.endmacro

fillJob .macro sourceByte, destination, length, chain
	.byte $0A ; 11 byte mode
	.byte $81, (\destination>>20) & $ff ; dest bank
	.byte $00 ; EOL
	.if \chain
        .byte dma.FILL|dma.CHAIN        ; fill, chain next job
	.else
		.byte dma.FILL ; Fill and last request
	.endif
	.word \length ; Size of Copy
	.word \sourceByte & $ff
	.byte $00
	.word \destination & $ffff
	.byte (\destination >> 16) & $f
	.word $0000
.endmacro


mixJob .macro source, destination, length, chain, backwards
	.byte $00 ; No more options
	.if \chain
		.byte $04 ; Mix and chain
	.else
		.byte $00 ; Mix and last request
	.endif	
	
	backByte .var 0
	.if \backwards
		.eval backByte = $40
		.eval \source = \source + \length - 1
		.eval \destination = \destination + \length - 1
	.endif
	.word \length ; Size of Copy
	.word \source & $ffff
	.byte (\source >> 16) + backByte
	.word \destination & $ffff
	.byte ((\destination >> 16) & $0f) + backByte
	.if \chain
		.word $0000
	.endif
.endmacro

.endnamespace ; dma

.endif