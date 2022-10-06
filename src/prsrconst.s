;---------------------------------------
; prsrconst.s
;---------------------------------------

prsla    = $01  ; a
prsua    = $41  ; A

prslbr   = $1b  ; [
prsrbr   = $1d  ; ]
prsfsl   = $2f   ; /


;---------------------------------------
; Address mode constants
;---------------------------------------

amimp    = 0
amacc    = 1
amimm    = 2
amzp     = 3
amzpx    = 4
amabs    = 5
amabx    = 6
amaby    = 7
amrel    = 8
amxin    = 9
aminy    = 10
amind    = 11
ambpr    = 12

;---------------------------------------
; LineType constants
;---------------------------------------

ltempty  = %00000000
ltlbdef  = %00000001
ltinstr  = %00000010
ltlbuse  = %00000100
ltdirtv  = %00001000
ltmcdef  = %00010000
ltmcuse  = %00100000
ltcomm   = %01000000
ltrelad  = %10000000

;---------------------------------------
; ValueSpec constants
;---------------------------------------

vshex    = 1*16
vsdec    = 2*16
vsbin    = 3*16
vsoct    = 4*16
vsexpr   = 5*16
vslabel  = 6*16
vschar   = 7*16

