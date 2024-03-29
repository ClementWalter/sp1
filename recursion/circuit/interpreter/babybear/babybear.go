package babybear

import (
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/emulated"
)

var MODULUS = new(big.Int).SetUint64(2013265921)

type Params struct{}

func (fp Params) NbLimbs() uint            { return 1 }
func (fp Params) BitsPerLimb() uint        { return 32 }
func (fp Params) IsPrime() bool            { return true }
func (fp Params) Modulus() *big.Int        { return MODULUS }
func (fp Params) NumElmsPerBN254Elm() uint { return 8 }

type Variable struct {
	Value *emulated.Element[Params]
}

type ExtensionVariable struct {
	value [4]*Variable
}

type Chip struct {
	api   frontend.API
	field *emulated.Field[Params]
}

func NewChip(api frontend.API) *Chip {
	field, err := emulated.NewField[Params](api)
	if err != nil {
		panic(err)
	}
	return &Chip{
		api:   api,
		field: field,
	}
}

func NewF(value string) *Variable {
	variable := emulated.ValueOf[Params](value)
	return &Variable{
		Value: &variable,
	}
}

func NewE(value []string) *ExtensionVariable {
	a := NewF(value[0])
	b := NewF(value[1])
	c := NewF(value[2])
	d := NewF(value[3])
	return &ExtensionVariable{value: [4]*Variable{a, b, c, d}}
}

func (c *Chip) AddF(a, b *Variable) *Variable {
	return &Variable{
		Value: c.field.Add(a.Value, b.Value),
	}
}

func (c *Chip) SubF(a, b *Variable) *Variable {
	return &Variable{
		Value: c.field.Sub(a.Value, b.Value),
	}
}

func (c *Chip) MulF(a, b *Variable) *Variable {
	return &Variable{
		Value: c.field.Mul(a.Value, b.Value),
	}
}

func (c *Chip) Neg(a *Variable) *Variable {
	return &Variable{
		Value: c.field.Neg(a.Value),
	}
}

func (c *Chip) Inv(a *Variable) *Variable {
	return &Variable{
		Value: c.field.Inverse(a.Value),
	}
}

func (c *Chip) AssertIsEqualV(a, b *Variable) {
	c.field.AssertIsEqual(a.Value, b.Value)
}

func (c *Chip) AssertNe(a, b *Variable) {
	diff := c.field.Sub(a.Value, b.Value)
	isZero := c.field.IsZero(diff)
	c.api.AssertIsEqual(isZero, frontend.Variable(0))
}

func (c *Chip) SelectF(cond frontend.Variable, a, b *Variable) *Variable {
	return &Variable{
		Value: c.field.Select(cond, a.Value, b.Value),
	}
}

func (c *Chip) AddEF(a *ExtensionVariable, b *Variable) *ExtensionVariable {
	v1 := c.AddF(a.value[0], b)
	v2 := c.AddF(a.value[1], NewF("0"))
	v3 := c.AddF(a.value[2], NewF("0"))
	v4 := c.AddF(a.value[3], NewF("0"))
	return &ExtensionVariable{value: [4]*Variable{v1, v2, v3, v4}}
}

func (c *Chip) AddE(a, b *ExtensionVariable) *ExtensionVariable {
	v1 := c.AddF(a.value[0], b.value[0])
	v2 := c.AddF(a.value[1], b.value[1])
	v3 := c.AddF(a.value[2], b.value[2])
	v4 := c.AddF(a.value[3], b.value[3])
	return &ExtensionVariable{value: [4]*Variable{v1, v2, v3, v4}}
}

func (c *Chip) SubE(a, b *ExtensionVariable) *ExtensionVariable {
	v1 := c.SubF(a.value[0], b.value[0])
	v2 := c.SubF(a.value[1], b.value[1])
	v3 := c.SubF(a.value[2], b.value[2])
	v4 := c.SubF(a.value[3], b.value[3])
	return &ExtensionVariable{value: [4]*Variable{v1, v2, v3, v4}}
}

func (c *Chip) MulE(a, b *ExtensionVariable) *ExtensionVariable {
	w := NewF("11")
	v := [4]*Variable{
		NewF("0"),
		NewF("0"),
		NewF("0"),
		NewF("0"),
	}

	for i := 0; i < 4; i++ {
		for j := 0; j < 4; j++ {
			if i+j >= 4 {
				v[i+j-4] = c.AddF(v[i+j-4], c.MulF(c.MulF(a.value[i], b.value[j]), w))
			} else {
				v[i+j] = c.AddF(v[i+j], c.MulF(a.value[i], b.value[j]))
			}
		}
	}

	return &ExtensionVariable{value: v}
}

func (c *Chip) DivE(a, b *ExtensionVariable) *ExtensionVariable {
	bInv := c.InvE(b)
	return c.MulE(a, bInv)
}

func (c *Chip) NegE(a *ExtensionVariable) *ExtensionVariable {
	v1 := c.Neg(a.value[0])
	v2 := c.Neg(a.value[1])
	v3 := c.Neg(a.value[2])
	v4 := c.Neg(a.value[3])
	return &ExtensionVariable{value: [4]*Variable{v1, v2, v3, v4}}
}

func (c *Chip) InvE(a *ExtensionVariable) *ExtensionVariable {
	v := [4]*Variable{
		NewF("0"),
		NewF("0"),
		NewF("0"),
		NewF("0"),
	}
	return &ExtensionVariable{value: v}
}

func (c *Chip) AssertIsEqualE(a, b *ExtensionVariable) {
	c.AssertIsEqualV(a.value[0], b.value[0])
	c.AssertIsEqualV(a.value[1], b.value[1])
	c.AssertIsEqualV(a.value[2], b.value[2])
	c.AssertIsEqualV(a.value[3], b.value[3])
}

func (c *Chip) AssertNeExtension(a, b *ExtensionVariable) {
	v1 := c.field.Sub(a.value[0].Value, b.value[0].Value)
	v2 := c.field.Sub(a.value[1].Value, b.value[1].Value)
	v3 := c.field.Sub(a.value[2].Value, b.value[2].Value)
	v4 := c.field.Sub(a.value[3].Value, b.value[3].Value)
	isZero1 := c.field.IsZero(v1)
	isZero2 := c.field.IsZero(v2)
	isZero3 := c.field.IsZero(v3)
	isZero4 := c.field.IsZero(v4)
	isZero1AndZero2 := c.api.And(isZero1, isZero2)
	isZero3AndZero4 := c.api.And(isZero3, isZero4)
	isZeroAll := c.api.And(isZero1AndZero2, isZero3AndZero4)
	c.api.AssertIsEqual(isZeroAll, frontend.Variable(0))
}

func (c *Chip) SelectExtension(cond frontend.Variable, a, b *ExtensionVariable) *ExtensionVariable {
	v1 := c.SelectF(cond, a.value[0], b.value[0])
	v2 := c.SelectF(cond, a.value[1], b.value[1])
	v3 := c.SelectF(cond, a.value[2], b.value[2])
	v4 := c.SelectF(cond, a.value[3], b.value[3])
	return &ExtensionVariable{value: [4]*Variable{v1, v2, v3, v4}}
}

func (c *Chip) SplitIntoBabyBear(in frontend.Variable) [8]Variable {
	bits := c.api.ToBinary(in)
	var result [8]frontend.Variable
	for i := 0; i < 8; i++ {
		result[i] = frontend.Variable(0)
	}

	for i := 0; i < 8; i++ {
		for j := 0; j < 8; j++ {
			if i*8+j <= 254 {
				result[i] = c.api.Add(result[i], c.api.Mul(bits[i*8+j], frontend.Variable(1<<j)))
			}
		}
	}

	var result2 [8]Variable
	for i := 0; i < 8; i++ {
		result2[i] = Variable{Value: c.field.NewElement(result[i])}
	}

	return result2
}

func (c *Chip) ToBinary(in *Variable) []frontend.Variable {
	return c.field.ToBits(c.field.Reduce(in.Value))
}

func (c *Chip) PrintF(in *Variable) {
	c.api.Println(c.field.Reduce(in.Value).Limbs[0])
}

func (c *Chip) PrintE(in *ExtensionVariable) {
	c.PrintF(in.value[0])
	c.PrintF(in.value[1])
	c.PrintF(in.value[2])
	c.PrintF(in.value[3])
}
