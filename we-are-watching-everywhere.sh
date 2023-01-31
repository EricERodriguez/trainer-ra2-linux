#!/bin/sh

RA2_1000_VA_1=0x4f23a8
RA2_1000_VA_2=0x556bfc
RA2_1000_VA_3=0x69e42c
RA2_1006_VA_1=0x4f2ff8
RA2_1006_VA_2=0x559f5c
RA2_1006_VA_3=0x6a8f3c

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
	set \$addr = (unsigned char *)\$arg0
	if \$addr[0] == 0x74 && \$addr[1] == 0x4a
		set *\$addr++ = 0x90
		set *\$addr = 0x90
		detach
		quit
	end
end
patch $RA2_1000_VA_1
patch $RA2_1006_VA_1
printf "\nGame version not supported or already patched\n"
detach
quit 1
EOT
