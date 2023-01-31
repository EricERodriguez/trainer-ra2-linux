#!/bin/sh

RA2_1000_VA=0x4e48f9
RA2_1006_VA=0x4e53a9
RA2MD_1000_VA=0x4f96d9

if [ $# != 1 ]; then
	printf "Usage: %s <pid>|auto\\n" "$0" 1>&2
	exit 255
fi

if [ "$1" = auto ]; then
	pid="`ps -A -o pid -o comm= | sed -En 's/^ *([0-9]+) +game(md)?\\.exe$/\\1/p'`"
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
define patch
	set \$addr = (unsigned char *)\$arg0
	if \$addr[0] == 0x2b && \$addr[1] == 0xc7
		set *\$addr++ = 0xeb
		set *\$addr = 0x58
		detach
		quit
	end
end
patch $RA2_1000_VA
patch $RA2_1006_VA
patch $RA2MD_1000_VA
printf "\nGame version not supported or already patched\n"
detach
quit 1
EOT
