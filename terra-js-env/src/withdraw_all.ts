import { Address } from "./terra_utils";
import { env } from "./env_helper";


function getContractLog(response: any){
	return response.logs[0].eventsByType.from_contract;
}


async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);
	let counter = new Address(env["mnemonics"][1]);
	console.log(counter.getAddress(), handler.getAddress());
	let cw20_tokens = env["cw20"];
	let cw20_token_names = Object.keys(cw20_tokens);
	let iliq_token_id = 0;
	// Uploading the contract code
	//let iliq_codeId: string[] = await handler.uploadContract("../artifacts/iliq_token.wasm");
	//let multisender_codeId: string[] = await handler.uploadContract("../artifacts/multisender.wasm");

	let iliq = handler.getContract(cw20_tokens[cw20_token_names[0]]);
	let iliq_counter = counter.getContract(cw20_tokens[cw20_token_names[0]]);
	let p2p = handler.getContract(env.contracts.p2p);
	let p2p_counter = counter.getContract(env.contracts.p2p);
	let fee = handler.getContract(env.contracts.fee);
	let fee_counter = counter.getContract(env.contracts.fee);

	let response: any; 

	// We try to add trades
	/*
	response = await p2p.execute.create_trade();
	response = await p2p.execute.create_trade();
	let trade_id = parseInt(getContractLog(response).trade_id[0]);
	console.log("Created trade",getContractLog(response));

	// We add funds
	// First we approve the contract for the amount
	let amount: string = "500";
	await iliq.execute.increase_allowance({
		spender: p2p.address,
		amount: amount
	});
	
	// Then we add the funds
	response = await p2p.execute.add_cw20({
		trade_id: 1,
		address: cw20_tokens[cw20_token_names[0]],
		amount: amount,
	});
	console.log("Added token", getContractLog(response));
	

	// We confirm our trade ! 
	response = await p2p.execute.confirm_trade({
		trade_id: 1,
	});

	// We create a counter trade 
	response = await p2p_counter.execute.suggest_counter_trade({
		trade_id: 1
	});
	*/
	
	// We query all our counter trades
	response = await p2p.query.get_all_counter_trades({
		filters:{
			owner:counter.getAddress(),
		}
	})
	for(let counter_trade of response.counter_trades){
		if(counter_trade.trade_info.state != "cancelled" && counter_trade.trade_info.state != "accepted")
		{
			let response = await p2p_counter.execute.cancel_counter_trade({
				trade_id: counter_trade.trade_id,
				counter_id: counter_trade.counter_id,
			})
			console.log("Cancelled Counter trade", getContractLog(response));
		}else{
			console.log("No cancel counter :/")
		}
	}


	// We query all our trades
	response = await p2p.query.get_all_trades({
		filters:{
			owner:handler.getAddress(),
		}
	})
	// We query the created trades
	
	for(let trade of response.trades){
		if(trade.trade_info.state != "cancelled" && trade.trade_info.state != "accepted")
		{
			let response = await p2p.execute.cancel_trade({
				trade_id: trade.trade_id
			})
			console.log("Cancelled trade", getContractLog(response));
		}else{
			console.log("No cancel :/")
		}
	}
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
