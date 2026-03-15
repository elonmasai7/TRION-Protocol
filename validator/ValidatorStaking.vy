# @version 0.3.10

interface ERC20:
    def transferFrom(sender: address, receiver: address, amount: uint256) -> bool: nonpayable
    def transfer(receiver: address, amount: uint256) -> bool: nonpayable

owner: public(address)
slasher: public(address)
stake_token: public(address)
treasury: public(address)

stakes: public(HashMap[address, uint256])

event Staked:
    validator: address
    amount: uint256

event Unstaked:
    validator: address
    amount: uint256

event Slashed:
    validator: address
    amount: uint256

event SlasherUpdated:
    slasher: address

@external
def __init__(token: address, treasury_addr: address):
    self.owner = msg.sender
    self.slasher = msg.sender
    self.stake_token = token
    self.treasury = treasury_addr

@external
def set_slasher(new_slasher: address):
    assert msg.sender == self.owner, "owner only"
    self.slasher = new_slasher
    log SlasherUpdated(new_slasher)

@external
def stake(amount: uint256):
    assert amount > 0, "amount=0"
    assert ERC20(self.stake_token).transferFrom(msg.sender, self, amount), "transfer failed"
    self.stakes[msg.sender] += amount
    log Staked(msg.sender, amount)

@external
def unstake(amount: uint256):
    assert amount > 0, "amount=0"
    assert self.stakes[msg.sender] >= amount, "insufficient stake"
    self.stakes[msg.sender] -= amount
    assert ERC20(self.stake_token).transfer(msg.sender, amount), "transfer failed"
    log Unstaked(msg.sender, amount)

@external
def slash(validator: address, amount: uint256):
    assert msg.sender == self.slasher, "slasher only"
    assert amount > 0, "amount=0"
    assert self.stakes[validator] >= amount, "insufficient stake"
    self.stakes[validator] -= amount
    assert ERC20(self.stake_token).transfer(self.treasury, amount), "transfer failed"
    log Slashed(validator, amount)
