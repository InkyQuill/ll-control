#!/bin/bash
# Demonstration script - shows live sensor data

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     L-Connect Linux - Real-time Sensor Demonstration        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

cd /home/inky/Development/lian-li/lconnect-linux

echo "Reading sensor data 5 times (1 second interval)..."
echo ""

for i in {1..5}; do
    # Parse the output
    result=$(./target/release/parse_test 2>/dev/null | grep -A 4 "C# HandshakeInfo:")

    if [ -n "$result" ]; then
        # Extract values
        fan=$(echo "$result" | grep "Fan RPM" | awk '{print $3}')
        pump=$(echo "$result" | grep "Pump RPM" | awk '{print $3}')
        temp=$(echo "$result" | grep "Temp:" | awk '{print $2}')

        # Format output
        printf "  [%d/5]  Fan: %6s RPM  |  Pump: %6s RPM  |  Temp: %s\n" \
            "$i" "$fan" "$pump" "$temp"
    fi

    if [ $i -lt 5 ]; then
        sleep 1
    fi
done

echo ""
echo "✅ Test complete!"
echo ""
echo "Ready to run GUI:"
echo "  ./target/release/lconnect_linux"
echo ""
