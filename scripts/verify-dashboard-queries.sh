#!/bin/bash

# Script to verify all dashboard queries work with available metrics

echo "Verifying CBC Dashboard Queries..."
echo ""

queries=(
    "cbc_consensus_current_epoch"
    "cbc_consensus_active_validators" 
    "rate(cbc_author_mismatch_total[5m])"
    "rate(cbc_epoch_transitions_total[1h])"
    "histogram_quantile(0.95, rate(cbc_block_production_time_seconds_bucket[5m]))"
    "substrate_rpc_sessions_opened"
    "substrate_rpc_sessions_closed"
    "cbc_consensus_total_reserved_stake"
    "cbc_consensus_total_rewards_distributed"
    "cbc_consensus_total_slashed_amount"
    "substrate_block_height{status=\"best\"}"
    "substrate_block_height{status=\"finalized\"}"
    "substrate_sub_libp2p_is_major_syncing"
    "substrate_sync_import_queue_blocks_submitted"
    "substrate_sync_extra_justifications{status=\"failed\"}"
)

for query in "${queries[@]}"; do
    echo "Testing: $query"
    result=$(curl -s "http://localhost:9090/api/v1/query?query=$(echo "$query" | sed 's/ /%20/g')" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data['status'] == 'success' and data['data']['result']:
        print('  SUCCESS - Value:', data['data']['result'][0]['value'][1])
    elif data['status'] == 'success':
        print('  SUCCESS but no data')
    else:
        print('  FAILED:', data.get('error', 'Unknown error'))
except Exception as e:
    print('  ERROR parsing response:', str(e))
")
    echo "$result"
    echo ""
done

echo "Dashboard query verification complete!"
echo ""
echo "Your dashboard should now work at: http://localhost:3000"
echo "   Username: admin"
echo "   Password: admin"