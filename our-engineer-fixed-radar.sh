#!/bin/sh

RA2_1000_VA_1=0x4f22ca
RA2_1000_VA_2=0x62a469
RA2_1006_VA_1=0x4f2f1a
RA2_1006_VA_2=0x632f39

if [ $# != 1 ]; then
	printf "Usage: %s <pid>|auto\\n" "$0" 1>&2
	exit 255
fi

if [ "$1" = auto ]; then
	pid="`ps -A -o pid -o comm= | sed -En 's/^ *([0-9]+) +game\\.exe$/\\1/p'`"
	if [ -z "$pid" ]; then
		echo "Cannot find a game.exe process" 1>&2
		exit 1
	fi
	pid="${pid%% 
*}"
else
	pid="$1"
fi

exec gdb --pid "$pid" << EOT
define patch
	set \$addr1 = (unsigned char *)\$arg0
	set \$addr2 = (unsigned char *)\$arg1
	if \$addr1[0] == 0x74 && \$addr1[1] == 0x49 && \$addr2[0] == 0x75 && \$addr2[1] == 0x5d
		set \$addr1[0] = 0x90
		set \$addr1[1] = 0x90
		set \$addr2[0] = 0x90
		set \$addr2[1] = 0x90
		detach
		quit
	end
end
patch $RA2_1000_VA_1 $RA2_1000_VA_2
patch $RA2_1006_VA_1 $RA2_1006_VA_2
printf "\nGame version not supported or already patched\n"
detach
quit 1
EOT
