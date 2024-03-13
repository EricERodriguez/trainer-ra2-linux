#!/bin/sh

RA2_1000_VA_1=0x49b8ac
RA2_1000_VA_2=0x49b8ba
RA2_1006_VA_1=0x49bc1c
RA2_1006_VA_2=0x49bc2a
RA2MD_1000_VA_1=0x4ab9bc
RA2MD_1000_VA_2=0x4ab9ca

if [ $# != 1 ]; then
	printf "Usage: %s <pid>|auto\\n" "$0" 1>&2
	exit 255
fi

if [ "$1" = auto ]; then
	pid="`ps -A -o pid -o comm= | sed -En 's/^ *([0-9]+) +game(md)?\\.exe\$/\\1/p'`"
	if [ -z "$pid" ]; then
		echo "Cannot find a game.exe or a gamemd.exe process" 1>&2
		exit 1
	fi
	pid="${pid%% 
*}"
else
	pid="$1"
fi

exec gdb --pid "$pid" << EOT
if *(unsigned int *)$RA2_1000_VA_1 == 0x01a4840f && *(unsigned int *)$RA2_1000_VA_2 == 0x0196840f
	printf "\\nRed Alert 2 version 1.000 detected\\n"
	set \$addr1 = (unsigned char *)$RA2_1000_VA_1
	set \$addr2 = (unsigned char *)$RA2_1000_VA_2
else
	if *(unsigned int *)$RA2_1006_VA_1 == 0x01c4840f && *(unsigned int *)$RA2_1006_VA_2 == 0x01b6840f
		printf "\\nRed Alert 2 version 1.006 detected\\n"
		set \$addr1 = (unsigned char *)$RA2_1006_VA_1
		set \$addr2 = (unsigned char *)$RA2_1006_VA_2
	else
		if *(unsigned int *)$RA2MD_1000_VA_1 == 0x01c4840f && *(unsigned int *)$RA2MD_1000_VA_2 == 0x01b6840f
			printf "\\nYuri's Revenge version 1.000 detected\\n"
			set \$addr1 = (unsigned char *)$RA2MD_1000_VA_1
			set \$addr2 = (unsigned char *)$RA2MD_1000_VA_2
		else
			printf "\\nGame version not supported or already patched\\n"
			detach
			quit 1
		end
	end
end
set *\$addr1++ = 0x90
set *\$addr1++ = 0x90
set *\$addr1++ = 0x90
set *\$addr1++ = 0x90
set *\$addr1++ = 0x90
set *\$addr1 = 0x90
set *\$addr2++ = 0x90
set *\$addr2++ = 0x90
set *\$addr2++ = 0x90
set *\$addr2++ = 0x90
set *\$addr2++ = 0x90
set *\$addr2 = 0x90
detach
quit
EOT
