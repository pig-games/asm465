     ; Based on the great work by Shallan at: https://github.com/smnjameson

.cpu "4510"

stabpqz  .function address ; sta [BP4],z
        nop
        sta (address),z
    .endfunction

ldqa .function address ; ldq bp/addr/(BP),z/[BP4],z
	neg
	neg
	lda address
.endfunction

stqa .function address ; stq bp/addr/(BP)/[BP4]
	neg
	neg
	sta address
.endfunction

adqa .function address ; adcq bp/addr/(BP)/[BP4]
	neg
	neg
	adc address
.endfunction

BasicUpstart65 .macro addr
		.byte $09,$20 ; End of command marker (first byte after the 00 terminator)
		.byte $0a,$00 ; 10
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
		sta $d02f
		lda #$96
		sta $d02f
.endmacro

enableVIC4Registers .macro
		lda #$00
		tax 
		tay 
		taz 
		map
		eom

		lda #$47	;Enable VIC IV
		sta $d02f
		lda #$53
		sta $d02f
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

mapMemory .macro source, target
	sourceMB .var (\source & $ff00000) >> 20
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffHi .var \sourceOffset >> 16
	sourceOffLo .var (\sourceOffset & $0ff00 ) >> 8
	bitLo .var pow(2, (((\target) & $ff00) >> 12) / 2) << 4
	bitHi .var pow(2, (((\target-$8000) & $ff00) >> 12) / 2) << 4
	
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
		ldx #[sourceOffHi + bitLo]
		ldy #$00
		ldz #$00
	.else
		lda #$00
		ldx #$00
		ldy #sourceOffLo
		ldz #[sourceOffHi + bitHi]
	.endif	
	map 
	eom
.endmacro

VIC4_SetCharLocation .macro addr
	lda #[\addr & $ff]
	sta $d068
	lda #[[\addr & $ff00]>>8]
	sta $d069
	lda #[[\addr & $ff0000]>>16]
	sta $d06a
.endmacro

VIC4_SetScreenLocation .macro addr
	lda #[\addr & $ff]
	sta $d060
	lda #[[\addr & $ff00]>>8]
	sta $d061
	lda #[[\addr & $ff0000]>>16]
	sta $d062
	lda #[[[\addr & $ff0000]>>24] & $0f]
	sta $d063
.endmacro

RunDMAJob .macro JobPointer
		lda #[\JobPointer >> 16]
		sta $d702
		sta $d704
		lda #>\JobPointer
		sta $d701
		lda #<\JobPointer
		sta $d705
.endmacro

DMAHeader .macro SourceBank, DestBank
		.byte $0A ; Request format is F018A
		.byte $80, \SourceBank
		.byte $81, \DestBank
.endmacro

DMAStep .macro SourceStep, SourceStepFractional, DestStep, DestStepFractional
		.if \SourceStepFractional != 0
			.byte $82, \SourceStepFractional
		.endif
		.if \SourceStep != 1
			.byte $83, \SourceStep
		.endif
		.if \DestStepFractional != 0
			.byte $84, \DestStepFractional
		.endif
		.if \DestStep != 1
			.byte $85, \DestStep
		.endif
.endmacro

DMADisableTransparency .macro
		.byte $06
.endmacro

DMAEnableTransparency .macro TransparentByte
		.byte $07 
		.byte $86, \TransparentByte
.endmacro

DMACopyJob .macro Source, Destination, Length, Chain, Backwards
	.byte $00 ; No more options
	.if \Chain
		.byte $04 ; Copy and chain
	.else
		.byte $00 ; Copy and last request
	.endif
	
	backByte .var 0
	.if \Backwards
		.eval backByte = $40
		.eval \Source = \Source + \Length - 1
		.eval \Destination = \Destination + \Length - 1
	.endif
	.word \Length ; Size of Copy

	.word \Source & $ffff
	.byte [\Source >> 16] + backByte

	.word \Destination & $ffff
	.byte [[\Destination >> 16] & $0f]  + backByte
	.if \Chain
		.word $0000
	.endif
.endmacro

DMAFillJob .macro SourceByte, Destination, Length, Chain
	.byte $00 ; No more options
	.if \Chain
	 	.byte $07 ; Fill and chain
	.else
		.byte $03 ; Fill and last request
	.endif
	.word \Length ; Size of Copy
	.word \SourceByte
	.byte $00
	.word \Destination & $ffff
	.byte [[\Destination >> 16] & $0f]
	.byte $00 ; command hi
	.word $0000
.endmacro


DMAMixJob .macro Source, Destination, Length, Chain, Backwards
	.byte $00 ; No more options
	.if \Chain
		.byte $04 ; Mix and chain
	.else
		.byte $00 ; Mix and last request
	.endif	
	
	backByte .var 0
	.if \Backwards
		.eval backByte = $40
		.eval \Source = \Source + \Length - 1
		.eval \Destination = \Destination + \Length - 1
	.endif
	.word \Length ; Size of Copy
	.word \Source & $ffff
	.byte [\Source >> 16] + backByte
	.word \Destination & $ffff
	.byte [[\Destination >> 16] & $0f]  + backByte
	.if \Chain
		.word $0000
	.endif
.endmacro
